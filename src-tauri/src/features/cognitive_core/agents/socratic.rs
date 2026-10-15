//! `/socratic` agent: interrogative tutor.
//!
//! Locks the system prompt so the model never answers directly — it
//! cross-references the user's claim against retrieved context and returns
//! exactly one probing question that exposes a gap, flaw, or missing evidence.

use tokio::sync::mpsc;

use super::{forward_stream, format_context};
use crate::features::inference::{GenerationParams, InferenceClient};
use crate::features::vector_db::VectorIndex;
use crate::shared::error::AppResult;
use crate::shared::models::{ChatMessage, RetrievedChunk, StreamEvent};

/// The immutable Socratic persona. Kept as a constant so the behavior can't be
/// diluted by per-request text.
pub const SOCRATIC_SYSTEM: &str = "\
You are a Socratic examiner. You NEVER provide answers, explanations, or \
solutions. Cross-reference the student's statement against the provided context \
to find a logical gap, unstated assumption, or missing evidence. Respond with \
EXACTLY ONE incisive question that forces the student to confront that gap. \
Output only the single question — no preamble, no follow-up.";

/// Retrieve context around the student's input and stream a single challenge.
pub async fn run(
    client: &InferenceClient,
    params: &GenerationParams,
    index: &VectorIndex,
    input: &str,
    tx: &mpsc::Sender<StreamEvent>,
) -> AppResult<()> {
    let chunks = index.retrieve(input).await.unwrap_or_default();
    let messages = build_prompt(input, &chunks);
    let rx = client.stream_chat(messages, params.clone()).await?;
    forward_stream(rx, tx).await
}

/// Build the Socratic message list: locked system persona + context, then the
/// student's statement.
pub fn build_prompt(input: &str, chunks: &[RetrievedChunk]) -> Vec<ChatMessage> {
    let system = format!("{SOCRATIC_SYSTEM}\n\nContext:\n{}", format_context(chunks));
    vec![ChatMessage::system(system), ChatMessage::user(input)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::models::Role;

    #[test]
    fn system_prompt_locks_persona() {
        let msgs = build_prompt("The earth is flat.", &[]);
        assert_eq!(msgs[0].role, Role::System);
        assert!(msgs[0].content.contains("EXACTLY ONE"));
        assert!(msgs[0].content.contains("NEVER provide answers"));
        assert_eq!(msgs[1].content, "The earth is flat.");
    }
}
