//! Command agents. Each agent consumes shared deps (inference client + vector
//! index) plus a payload and streams [`StreamEvent`]s into a channel. The router
//! appends the terminal `Done`/`Error` event, so agents only emit content.

pub mod planner;
pub mod rag;
pub mod researcher;
pub mod socratic;

use tokio::sync::mpsc;

use crate::shared::error::AppResult;
use crate::shared::models::{RetrievedChunk, StreamEvent};

/// Forward a token stream from the inference client to the UI channel as
/// `Token` events. Stops early (Ok) if the consumer has hung up.
pub(crate) async fn forward_stream(
    mut rx: mpsc::Receiver<AppResult<String>>,
    tx: &mpsc::Sender<StreamEvent>,
) -> AppResult<()> {
    while let Some(item) = rx.recv().await {
        let text = item?;
        if tx.send(StreamEvent::Token { text }).await.is_err() {
            break;
        }
    }
    Ok(())
}

/// Emit a progress log line, ignoring a dropped consumer.
pub(crate) async fn log(tx: &mpsc::Sender<StreamEvent>, line: impl Into<String>) {
    let _ = tx.send(StreamEvent::Log { line: line.into() }).await;
}

/// Render retrieved chunks into a numbered context block for prompt injection.
pub(crate) fn format_context(chunks: &[RetrievedChunk]) -> String {
    if chunks.is_empty() {
        return "(no relevant documents found in the workspace)".to_string();
    }
    chunks
        .iter()
        .enumerate()
        .map(|(i, c)| format!("[{}] source: {}\n{}", i + 1, c.metadata.source_file, c.text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::models::ChunkMetadata;

    fn chunk(src: &str, text: &str) -> RetrievedChunk {
        RetrievedChunk {
            text: text.to_string(),
            metadata: ChunkMetadata {
                source_file: src.to_string(),
                timestamp: 0,
                chunk_id: format!("{src}#0"),
            },
            score: 0.1,
        }
    }

    #[test]
    fn empty_context_is_labeled() {
        assert!(format_context(&[]).contains("no relevant documents"));
    }

    #[test]
    fn context_numbers_and_cites_sources() {
        let ctx = format_context(&[chunk("a.md", "alpha"), chunk("b.pdf", "beta")]);
        assert!(ctx.contains("[1] source: a.md"));
        assert!(ctx.contains("alpha"));
        assert!(ctx.contains("[2] source: b.pdf"));
        assert!(ctx.contains("beta"));
    }
}
