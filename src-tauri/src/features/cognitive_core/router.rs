//! Command router: parses the leading command token and dispatches to an agent.
//!
//! The parse step is pure and exhaustively tested; [`Router::handle`] wires the
//! shared deps into the selected agent and lets the caller append the terminal
//! stream event.

use std::path::Path;

use tokio::sync::mpsc;

use super::agents;
use crate::features::inference::{extract_cards, GenerationParams, HiggsClient};
use crate::features::ingestion;
use crate::features::vector_db::VectorIndex;
use crate::shared::error::AppResult;
use crate::shared::models::{Card, ChatMessage, StreamEvent};

/// The recognized command states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// No command — retrieval-augmented chat.
    Chat,
    /// `/plan` — study syllabus synthesis.
    Plan,
    /// `/deep-research` — iterative investigation.
    DeepResearch,
    /// `/socratic` — interrogative tutor.
    Socratic,
}

/// A parsed user input: the command and its remaining payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedInput {
    pub command: Command,
    pub payload: String,
}

/// Parse the leading token of `input` into a [`Command`] + payload. Unknown or
/// absent commands fall through to [`Command::Chat`] with the full input as the
/// query.
pub fn parse(input: &str) -> ParsedInput {
    let trimmed = input.trim();
    let (head, tail) = match trimmed.split_once(char::is_whitespace) {
        Some((h, t)) => (h, t.trim()),
        None => (trimmed, ""),
    };

    match head {
        "/plan" => ParsedInput { command: Command::Plan, payload: tail.to_string() },
        "/deep-research" => {
            ParsedInput { command: Command::DeepResearch, payload: tail.to_string() }
        }
        "/socratic" => ParsedInput { command: Command::Socratic, payload: tail.to_string() },
        // Unknown "/foo" or plain text: treat the whole input as a chat query.
        _ => ParsedInput { command: Command::Chat, payload: trimmed.to_string() },
    }
}

/// Holds the shared subsystems and routes a request to the right agent.
pub struct Router {
    client: HiggsClient,
    index: VectorIndex,
}

impl Router {
    pub fn new(client: HiggsClient, index: VectorIndex) -> Self {
        Self { client, index }
    }

    /// Parse `input` and run the matching agent, streaming content events into
    /// `tx`. The caller is responsible for the terminal `Done`/`Error` event.
    pub async fn handle(
        &self,
        input: &str,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> AppResult<()> {
        let ParsedInput { command, payload } = parse(input);
        match command {
            Command::Chat => agents::rag::run(&self.client, &self.index, &payload, tx).await,
            Command::Plan => agents::planner::run(&self.client, &self.index, &payload, tx).await,
            Command::DeepResearch => {
                agents::researcher::run(&self.client, &self.index, &payload, tx).await
            }
            Command::Socratic => {
                agents::socratic::run(&self.client, &self.index, &payload, tx).await
            }
        }
    }

    /// Ingest a local file into the shared index. Returns the chunk count.
    pub async fn ingest_file(&self, path: impl AsRef<Path>) -> AppResult<usize> {
        let chunks = ingestion::ingest_path(path).await?;
        let n = chunks.len();
        self.index.index(&chunks).await?;
        Ok(n)
    }

    /// Scrape and ingest a web page into the shared index.
    pub async fn ingest_web(&self, url: &str) -> AppResult<usize> {
        let chunks = ingestion::ingest_web(url).await?;
        let n = chunks.len();
        self.index.index(&chunks).await?;
        Ok(n)
    }

    /// Fetch and ingest a YouTube transcript into the shared index.
    pub async fn ingest_youtube(&self, url: &str) -> AppResult<usize> {
        let chunks = ingestion::ingest_youtube(url).await?;
        let n = chunks.len();
        self.index.index(&chunks).await?;
        Ok(n)
    }

    /// Generate Anki flashcards grounded in retrieved context for `topic`.
    /// The cards are staged (returned) for the user to review before syncing.
    pub async fn generate_cards(&self, topic: &str) -> AppResult<Vec<Card>> {
        let chunks = self.index.retrieve(topic).await.unwrap_or_default();
        let context = agents::format_context(&chunks);
        let system = "You create high-quality Anki flashcards from the provided \
            context. Each card is one focused question (front) with a concise \
            answer (back). Prefer atomic facts over compound ones. Add a few \
            relevant lowercase tags per card.";
        let user = format!("Topic: {topic}\n\nContext:\n{context}");
        let messages = vec![ChatMessage::system(system), ChatMessage::user(user)];
        extract_cards(&self.client, messages, GenerationParams::default()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_chat_with_full_query() {
        let p = parse("what is entropy?");
        assert_eq!(p.command, Command::Chat);
        assert_eq!(p.payload, "what is entropy?");
    }

    #[test]
    fn parses_each_command_and_strips_it() {
        assert_eq!(parse("/plan thermodynamics").command, Command::Plan);
        assert_eq!(parse("/plan thermodynamics").payload, "thermodynamics");
        assert_eq!(parse("/deep-research black holes").command, Command::DeepResearch);
        assert_eq!(parse("/deep-research black holes").payload, "black holes");
        assert_eq!(parse("/socratic my thesis").command, Command::Socratic);
        assert_eq!(parse("/socratic my thesis").payload, "my thesis");
    }

    #[test]
    fn command_without_payload() {
        let p = parse("/plan");
        assert_eq!(p.command, Command::Plan);
        assert_eq!(p.payload, "");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let p = parse("   /socratic   claim here   ");
        assert_eq!(p.command, Command::Socratic);
        assert_eq!(p.payload, "claim here");
    }

    #[test]
    fn unknown_slash_command_is_chat() {
        let p = parse("/unknown do a thing");
        assert_eq!(p.command, Command::Chat);
        assert_eq!(p.payload, "/unknown do a thing");
    }

    #[test]
    fn command_prefix_must_be_exact_token() {
        // "/planning" is not "/plan"
        let p = parse("/planning my week");
        assert_eq!(p.command, Command::Chat);
    }

    #[test]
    fn empty_input_is_empty_chat() {
        let p = parse("   ");
        assert_eq!(p.command, Command::Chat);
        assert_eq!(p.payload, "");
    }
}
