//! `/plan` agent: study-syllabus synthesizer.
//!
//! Aggregates workspace metadata (chunk counts, distinct sources) and a sample
//! of content, then streams a structured markdown study plan. Conversational
//! chat framing is dropped — the output is a document.

use std::collections::BTreeMap;

use tokio::sync::mpsc;

use super::{forward_stream, log};
use crate::features::inference::{GenerationParams, HiggsClient};
use crate::features::vector_db::VectorIndex;
use crate::shared::error::AppResult;
use crate::shared::models::{ChatMessage, DocumentChunk, StreamEvent};

/// Cap on chunks scanned for metrics/sampling — bounds prompt size on large
/// workspaces.
const SCAN_LIMIT: usize = 2000;
/// Number of sample excerpts fed to the model for grounding.
const SAMPLE_SIZE: usize = 12;

/// Aggregated view of the ingested workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMetrics {
    pub total_chunks: usize,
    /// (source_file, chunk_count), sorted by source name.
    pub sources: Vec<(String, usize)>,
}

/// Scan the workspace, compute metrics, and stream a markdown syllabus.
pub async fn run(
    client: &HiggsClient,
    index: &VectorIndex,
    topic: &str,
    tx: &mpsc::Sender<StreamEvent>,
) -> AppResult<()> {
    log(tx, "Scanning workspace…").await;
    let chunks = index.scan(SCAN_LIMIT).await.unwrap_or_default();
    let metrics = compute_metrics(&chunks);
    log(
        tx,
        format!(
            "Found {} chunks across {} sources.",
            metrics.total_chunks,
            metrics.sources.len()
        ),
    )
    .await;

    let sample: Vec<&DocumentChunk> = chunks.iter().take(SAMPLE_SIZE).collect();
    let messages = build_prompt(topic, &metrics, &sample);
    let rx = client.stream_chat(messages, GenerationParams { temperature: 0.4, ..Default::default() }).await?;
    forward_stream(rx, tx).await
}

/// Aggregate chunk counts per source across the workspace.
pub fn compute_metrics(chunks: &[DocumentChunk]) -> WorkspaceMetrics {
    let mut per_source: BTreeMap<String, usize> = BTreeMap::new();
    for c in chunks {
        *per_source.entry(c.metadata.source_file.clone()).or_insert(0) += 1;
    }
    WorkspaceMetrics {
        total_chunks: chunks.len(),
        sources: per_source.into_iter().collect(),
    }
}

/// Build the planning prompt from metrics + a content sample.
pub fn build_prompt(
    topic: &str,
    metrics: &WorkspaceMetrics,
    sample: &[&DocumentChunk],
) -> Vec<ChatMessage> {
    let source_list = if metrics.sources.is_empty() {
        "(workspace is empty)".to_string()
    } else {
        metrics
            .sources
            .iter()
            .map(|(s, n)| format!("- {s} ({n} chunks)"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let excerpts = sample
        .iter()
        .map(|c| format!("- ({}) {}", c.metadata.source_file, first_line(&c.text)))
        .collect::<Vec<_>>()
        .join("\n");

    let focus = if topic.trim().is_empty() {
        "the entire workspace".to_string()
    } else {
        format!("the topic \"{topic}\"")
    };

    let system = "You are whis, a curriculum designer. Produce a structured \
        markdown study plan: a title, an ordered list of study modules with \
        subtopics, and a final \"Gaps & Next Steps\" section calling out what the \
        materials do not yet cover. Output only markdown — no conversational \
        preamble."
        .to_string();

    let user = format!(
        "Design a study plan focused on {focus}.\n\n\
         Workspace sources:\n{source_list}\n\n\
         Total chunks: {}\n\n\
         Sample excerpts:\n{excerpts}",
        metrics.total_chunks
    );

    vec![ChatMessage::system(system), ChatMessage::user(user)]
}

/// First non-empty line of a chunk, truncated for the excerpt list.
fn first_line(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    if line.chars().count() > 160 {
        let truncated: String = line.chars().take(160).collect();
        format!("{truncated}…")
    } else {
        line.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::models::ChunkMetadata;

    fn chunk(src: &str, text: &str) -> DocumentChunk {
        DocumentChunk {
            text: text.to_string(),
            metadata: ChunkMetadata {
                source_file: src.to_string(),
                timestamp: 0,
                chunk_id: format!("{src}#x"),
            },
        }
    }

    #[test]
    fn metrics_count_per_source_sorted() {
        let chunks = vec![
            chunk("b.md", "1"),
            chunk("a.md", "2"),
            chunk("a.md", "3"),
        ];
        let m = compute_metrics(&chunks);
        assert_eq!(m.total_chunks, 3);
        assert_eq!(m.sources, vec![("a.md".into(), 2), ("b.md".into(), 1)]);
    }

    #[test]
    fn empty_workspace_metrics() {
        let m = compute_metrics(&[]);
        assert_eq!(m.total_chunks, 0);
        assert!(m.sources.is_empty());
    }

    #[test]
    fn prompt_lists_sources_and_focus() {
        let m = compute_metrics(&[chunk("a.md", "alpha topic")]);
        let sample = vec![];
        let msgs = build_prompt("photosynthesis", &m, &sample);
        assert!(msgs[1].content.contains("photosynthesis"));
        assert!(msgs[1].content.contains("a.md (1 chunks)"));
    }

    #[test]
    fn empty_topic_plans_whole_workspace() {
        let m = compute_metrics(&[]);
        let msgs = build_prompt("  ", &m, &[]);
        assert!(msgs[1].content.contains("entire workspace"));
        assert!(msgs[1].content.contains("workspace is empty"));
    }
}
