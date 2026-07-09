//! Embedded vector database subsystem.
//!
//! Combines the candle [`Embedder`] with the LanceDB [`VectorStore`]. The two
//! are independent (the store takes precomputed vectors), so the store is
//! testable without loading the model. [`VectorIndex`] is the convenience layer
//! that wires them together for the common ingest/retrieve paths.

pub mod embedder;
pub mod store;

pub use embedder::Embedder;
pub use store::VectorStore;

use std::path::Path;

use crate::shared::config::TOP_K;
use crate::shared::error::AppResult;
use crate::shared::models::{DocumentChunk, RetrievedChunk};

/// High-level index: embeds text with the local model and persists / queries it
/// through LanceDB.
pub struct VectorIndex {
    embedder: Embedder,
    store: VectorStore,
}

impl VectorIndex {
    /// Load the embedding model and open the database at `db_path`.
    pub async fn open(db_path: impl AsRef<Path>) -> AppResult<Self> {
        let embedder = Embedder::load().await?;
        let store = VectorStore::connect(db_path).await?;
        Ok(Self { embedder, store })
    }

    /// Embed and store a batch of chunks.
    pub async fn index(&self, chunks: &[DocumentChunk]) -> AppResult<()> {
        let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
        let embeddings = self.embedder.embed(&texts)?;
        self.store.insert(chunks, &embeddings).await
    }

    /// Embed `query` and return the top [`TOP_K`] most similar chunks.
    pub async fn retrieve(&self, query: &str) -> AppResult<Vec<RetrievedChunk>> {
        let embedding = self.embedder.embed_one(query)?;
        self.store.search(&embedding, TOP_K).await
    }

    /// Up to `limit` stored chunks without ranking (for workspace metrics).
    pub async fn scan(&self, limit: usize) -> AppResult<Vec<DocumentChunk>> {
        self.store.scan(limit).await
    }
}
