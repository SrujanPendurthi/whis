//! Default (no-command) agent: retrieval-augmented generation.
//!
//! Retrieves the most relevant chunks, injects them as grounded context, and
//! streams the model's answer.

use tokio::sync::mpsc;

use super::{forward_stream, format_context};
use crate::features::inference::{GenerationParams, InferenceClient};
use crate::features::vector_db::VectorIndex;
use crate::shared::error::AppResult;
use crate::shared::models::{ChatMessage, RetrievedChunk, StreamEvent};

/// Retrieve context for `query`, then stream a grounded answer.
pub async fn run(
    client: &InferenceClient,
    params: &GenerationParams,
    index: &VectorIndex,
    query: &str,
    tx: &mpsc::Sender<StreamEvent>,
) -> AppResult<()> {
    // Retrieval failures (e.g. empty store) degrade to context-free answering.
    let chunks = index.retrieve(query).await.unwrap_or_default();
    let messages = build_prompt(query, &chunks);
    // Conversational chat honors the profile's temperature.
    let rx = client.stream_chat(messages, params.clone()).await?;
    forward_stream(rx, tx).await
}

/// Build the RAG message list: a grounding system prompt with injected context
/// followed by the user's question.
pub fn build_prompt(query: &str, chunks: &[RetrievedChunk]) -> Vec<ChatMessage> {
    let system = format!(
        "You are whis, a precise study assistant. Answer using the provided \
         context from the user's documents. If the context is insufficient, say \
         so and answer from general knowledge, marking that clearly. Cite \
         sources by their bracket number when you use them.\n\n\
         Context:\n{}",
        format_context(chunks)
    );
    vec![ChatMessage::system(system), ChatMessage::user(query)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::models::{ChunkMetadata, Role};

    fn chunk(text: &str) -> RetrievedChunk {
        RetrievedChunk {
            text: text.to_string(),
            metadata: ChunkMetadata {
                source_file: "notes.md".into(),
                timestamp: 0,
                chunk_id: "notes.md#0".into(),
            },
            score: 0.2,
        }
    }

    #[test]
    fn prompt_has_system_then_user() {
        let msgs = build_prompt("what is X?", &[chunk("X is a thing")]);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role, Role::System);
        assert_eq!(msgs[1].role, Role::User);
        assert_eq!(msgs[1].content, "what is X?");
    }

    #[test]
    fn system_prompt_injects_context() {
        let msgs = build_prompt("q", &[chunk("grounding fact")]);
        assert!(msgs[0].content.contains("grounding fact"));
        assert!(msgs[0].content.contains("source: notes.md"));
    }

    #[test]
    fn handles_no_context() {
        let msgs = build_prompt("q", &[]);
        assert!(msgs[0].content.contains("no relevant documents"));
    }
}
