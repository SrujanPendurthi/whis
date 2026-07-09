//! `/deep-research` agent: multi-step investigative loop.
//!
//! Decomposes a topic into sub-questions (via the routing model's structured
//! output), gathers evidence for each from the vector index while streaming
//! progress logs, then synthesizes a unified markdown report.
//!
//! Web scraping is an intended extension point: when a search provider is wired
//! in, discovered URLs would be scraped (`ingestion::ingest_web`) and indexed
//! before the retrieval step. For now the local corpus is the search space.

use serde_json::json;
use tokio::sync::mpsc;

use super::{forward_stream, format_context, log};
use crate::features::inference::{GenerationParams, HiggsClient};
use crate::features::vector_db::VectorIndex;
use crate::shared::error::AppResult;
use crate::shared::models::{ChatMessage, StreamEvent};

/// How many sub-questions to investigate.
const MAX_SUBQUESTIONS: usize = 5;

/// Run the research loop for `topic`.
pub async fn run(
    client: &HiggsClient,
    index: &VectorIndex,
    topic: &str,
    tx: &mpsc::Sender<StreamEvent>,
) -> AppResult<()> {
    if topic.trim().is_empty() {
        return Err(crate::shared::error::AppError::Inference(
            "/deep-research requires a topic".into(),
        ));
    }

    log(tx, format!("Planning research for: {topic}")).await;
    let questions = decompose(client, topic).await.unwrap_or_else(|_| vec![topic.to_string()]);

    let mut findings: Vec<(String, String)> = Vec::new();
    for (i, q) in questions.iter().enumerate() {
        log(tx, format!("[{}/{}] Investigating: {q}", i + 1, questions.len())).await;
        let chunks = index.retrieve(q).await.unwrap_or_default();
        log(tx, format!("    ↳ {} relevant passage(s)", chunks.len())).await;
        findings.push((q.clone(), format_context(&chunks)));
    }

    log(tx, "Synthesizing report…").await;
    let messages = build_report_prompt(topic, &findings);
    let rx = client
        .stream_chat(messages, GenerationParams { temperature: 0.5, ..Default::default() })
        .await?;
    forward_stream(rx, tx).await
}

/// Ask the model to break `topic` into focused sub-questions (structured output).
async fn decompose(client: &HiggsClient, topic: &str) -> AppResult<Vec<String>> {
    let schema = json!({
        "type": "object",
        "properties": {
            "questions": {
                "type": "array",
                "items": { "type": "string" },
                "maxItems": MAX_SUBQUESTIONS
            }
        },
        "required": ["questions"],
        "additionalProperties": false
    });
    let messages = vec![
        ChatMessage::system(
            "You plan research. Break the topic into at most 5 specific, \
             non-overlapping sub-questions that together cover it.",
        ),
        ChatMessage::user(format!("Topic: {topic}")),
    ];
    let value = client
        .complete_structured(messages, GenerationParams::default(), "research_plan", schema)
        .await?;
    let questions = value
        .get("questions")
        .and_then(|q| q.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .take(MAX_SUBQUESTIONS)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if questions.is_empty() {
        Ok(vec![topic.to_string()])
    } else {
        Ok(questions)
    }
}

/// Build the synthesis prompt from gathered per-question evidence.
pub fn build_report_prompt(topic: &str, findings: &[(String, String)]) -> Vec<ChatMessage> {
    let evidence = findings
        .iter()
        .map(|(q, ctx)| format!("### {q}\n{ctx}"))
        .collect::<Vec<_>>()
        .join("\n\n");

    let system = "You are whis, a research analyst. Using only the gathered \
        evidence, write a unified markdown deep-dive report: an overview, one \
        section per sub-question, and a \"Open Questions\" section for anything \
        the evidence did not resolve. Cite sources by their bracket numbers."
        .to_string();

    let user = format!("Topic: {topic}\n\nGathered evidence:\n\n{evidence}");
    vec![ChatMessage::system(system), ChatMessage::user(user)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::models::Role;

    #[test]
    fn report_prompt_includes_questions_and_evidence() {
        let findings = vec![
            ("What is X?".to_string(), "[1] source: a.md\nX detail".to_string()),
            ("Why Y?".to_string(), "(no relevant documents found in the workspace)".to_string()),
        ];
        let msgs = build_report_prompt("X and Y", &findings);
        assert_eq!(msgs[0].role, Role::System);
        assert!(msgs[1].content.contains("### What is X?"));
        assert!(msgs[1].content.contains("X detail"));
        assert!(msgs[1].content.contains("### Why Y?"));
        assert!(msgs[1].content.contains("Topic: X and Y"));
    }
}
