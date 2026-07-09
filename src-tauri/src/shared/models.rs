//! Core shared types and IPC payload structs.
//!
//! These types cross the Rust <-> TypeScript boundary. Every struct uses
//! `camelCase` on the wire so the matching TS interfaces read idiomatically.
//! Keep the TS mirror in `src/shared/types.ts` in sync with this file.

use serde::{Deserialize, Serialize};

/// Author of a chat turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single conversation turn sent to / received from the inference engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

/// Metadata attached to every stored chunk in the vector DB.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkMetadata {
    /// Absolute path or URL the chunk was derived from.
    pub source_file: String,
    /// Unix epoch seconds when the chunk was ingested.
    pub timestamp: i64,
    /// Stable per-chunk identifier (unique within a source).
    pub chunk_id: String,
}

/// A text chunk ready to be embedded and inserted into the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub text: String,
    #[serde(flatten)]
    pub metadata: ChunkMetadata,
}

/// A chunk returned from a similarity search, with its distance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub text: String,
    #[serde(flatten)]
    pub metadata: ChunkMetadata,
    /// Similarity distance (lower is closer for L2).
    pub score: f32,
}

/// A distinct ingested source and how many chunks it contributed. Powers the
/// workspace sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub source_file: String,
    pub chunk_count: usize,
}

/// A flashcard extracted for Anki. Mirrors the strict JSON schema the model is
/// constrained to when generating cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub front: String,
    pub back: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Incremental event streamed to the frontend as the model produces output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamEvent {
    /// A token / text delta appended to the current assistant message.
    Token { text: String },
    /// A human-readable progress log line (used by `/deep-research`).
    Log { line: String },
    /// The stream finished successfully.
    Done,
    /// The stream terminated with an error message.
    Error { message: String },
}
