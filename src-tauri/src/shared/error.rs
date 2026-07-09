//! Centralized error type for the whis backend.
//!
//! Every fallible feature surfaces failures as [`AppError`]. It implements
//! `serde::Serialize` so errors can cross the Tauri IPC boundary and reach the
//! frontend as plain strings without leaking internal panics.

use serde::{Serialize, Serializer};

/// The single error type returned across feature and IPC boundaries.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Failure talking to the local inference server (Higgs @ :8080).
    #[error("inference error: {0}")]
    Inference(String),

    /// Failure in the embedded vector database / embedding pipeline.
    #[error("vector store error: {0}")]
    VectorStore(String),

    /// Failure ingesting or parsing a document / remote source.
    #[error("ingestion error: {0}")]
    Ingestion(String),

    /// Failure talking to the local AnkiConnect endpoint (@ :8765).
    #[error("anki error: {0}")]
    Anki(String),

    /// Underlying HTTP transport failure.
    #[error("http error: {0}")]
    Http(String),

    /// (De)serialization failure of a JSON payload.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Filesystem / configuration / path resolution failure.
    #[error("config error: {0}")]
    Config(String),
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Http(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Serialization(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Config(e.to_string())
    }
}

/// Serialize as the human-readable message so the frontend receives a string,
/// never a tagged enum it would need to know the shape of.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Convenience alias used throughout the backend.
pub type AppResult<T> = Result<T, AppError>;
