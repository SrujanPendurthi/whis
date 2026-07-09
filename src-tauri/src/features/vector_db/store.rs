//! Embedded LanceDB store: connection management, chunk insertion, and top-k
//! vector similarity search.
//!
//! The database directory is always supplied by the caller ([`VectorStore::connect`]);
//! nothing here hardcodes a path. A sensible fallback lives in
//! `shared::config::default_db_path` for callers that don't have one.

use std::path::Path;
use std::sync::Arc;

use arrow_array::{
    Array, FixedSizeListArray, Float32Array, Int64Array, RecordBatch, RecordBatchIterator,
    RecordBatchReader, StringArray,
};
use arrow_schema::{DataType, Field, Schema};
use futures_util::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};
use lancedb::{Connection, Table};

use crate::shared::config::EMBEDDING_DIM;
use crate::shared::error::{AppError, AppResult};
use crate::shared::models::{ChunkMetadata, DocumentChunk, RetrievedChunk};

/// Single table holding all ingested chunks.
const TABLE_NAME: &str = "chunks";
/// Distance column LanceDB appends to vector-search results.
const DISTANCE_COL: &str = "_distance";

/// Handle to the embedded vector database.
pub struct VectorStore {
    conn: Connection,
}

impl VectorStore {
    /// Open (creating if absent) a LanceDB database at `db_path`. The path is
    /// caller-supplied and may point anywhere on disk.
    pub async fn connect(db_path: impl AsRef<Path>) -> AppResult<Self> {
        let uri = db_path.as_ref().to_string_lossy().to_string();
        let conn = lancedb::connect(&uri)
            .execute()
            .await
            .map_err(lance_err)?;
        Ok(Self { conn })
    }

    /// Insert chunks together with their precomputed embeddings. The two slices
    /// must be the same length and each embedding must be [`EMBEDDING_DIM`] long.
    pub async fn insert(
        &self,
        chunks: &[DocumentChunk],
        embeddings: &[Vec<f32>],
    ) -> AppResult<()> {
        if chunks.len() != embeddings.len() {
            return Err(AppError::VectorStore(format!(
                "chunk/embedding count mismatch: {} vs {}",
                chunks.len(),
                embeddings.len()
            )));
        }
        if chunks.is_empty() {
            return Ok(());
        }

        let batch = build_batch(chunks, embeddings)?;
        let schema = batch.schema();
        let table = self.ensure_table(&schema).await?;

        let reader: Box<dyn RecordBatchReader + Send> =
            Box::new(RecordBatchIterator::new(vec![Ok(batch)], schema));
        table.add(reader).execute().await.map_err(lance_err)?;
        Ok(())
    }

    /// Return the `k` nearest chunks to `query_embedding`, closest first.
    pub async fn search(
        &self,
        query_embedding: &[f32],
        k: usize,
    ) -> AppResult<Vec<RetrievedChunk>> {
        let names = self.conn.table_names().execute().await.map_err(lance_err)?;
        if !names.iter().any(|n| n == TABLE_NAME) {
            return Ok(Vec::new()); // nothing ingested yet
        }

        let table = self
            .conn
            .open_table(TABLE_NAME)
            .execute()
            .await
            .map_err(lance_err)?;

        let stream = table
            .query()
            .nearest_to(query_embedding)
            .map_err(lance_err)?
            .limit(k)
            .execute()
            .await
            .map_err(lance_err)?;

        let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(lance_err)?;
        let mut out = Vec::new();
        for batch in &batches {
            out.extend(rows_from_batch(batch)?);
        }
        Ok(out)
    }

    /// Return up to `limit` stored chunks without similarity ranking — used for
    /// workspace-level metrics (counts, distinct sources). Empty if nothing has
    /// been ingested yet.
    pub async fn scan(&self, limit: usize) -> AppResult<Vec<DocumentChunk>> {
        let names = self.conn.table_names().execute().await.map_err(lance_err)?;
        if !names.iter().any(|n| n == TABLE_NAME) {
            return Ok(Vec::new());
        }
        let table = self
            .conn
            .open_table(TABLE_NAME)
            .execute()
            .await
            .map_err(lance_err)?;

        let stream = table.query().limit(limit).execute().await.map_err(lance_err)?;
        let batches: Vec<RecordBatch> = stream.try_collect().await.map_err(lance_err)?;
        let mut out = Vec::new();
        for batch in &batches {
            out.extend(chunk_rows_from_batch(batch)?);
        }
        Ok(out)
    }

    /// Open the chunks table, creating an empty one with `schema` if needed.
    async fn ensure_table(&self, schema: &Arc<Schema>) -> AppResult<Table> {
        let names = self.conn.table_names().execute().await.map_err(lance_err)?;
        if names.iter().any(|n| n == TABLE_NAME) {
            self.conn.open_table(TABLE_NAME).execute().await.map_err(lance_err)
        } else {
            self.conn
                .create_empty_table(TABLE_NAME, schema.clone())
                .execute()
                .await
                .map_err(lance_err)
        }
    }
}

/// Arrow schema for the chunks table.
fn chunks_schema() -> Arc<Schema> {
    let item = Arc::new(Field::new("item", DataType::Float32, true));
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("source_file", DataType::Utf8, false),
        Field::new("timestamp", DataType::Int64, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(item, EMBEDDING_DIM as i32),
            false,
        ),
    ]))
}

/// Build a single RecordBatch from parallel chunk + embedding slices.
fn build_batch(chunks: &[DocumentChunk], embeddings: &[Vec<f32>]) -> AppResult<RecordBatch> {
    let ids = StringArray::from_iter_values(chunks.iter().map(|c| c.metadata.chunk_id.as_str()));
    let texts = StringArray::from_iter_values(chunks.iter().map(|c| c.text.as_str()));
    let sources =
        StringArray::from_iter_values(chunks.iter().map(|c| c.metadata.source_file.as_str()));
    let timestamps = Int64Array::from_iter_values(chunks.iter().map(|c| c.metadata.timestamp));

    let mut flat = Vec::with_capacity(chunks.len() * EMBEDDING_DIM);
    for emb in embeddings {
        if emb.len() != EMBEDDING_DIM {
            return Err(AppError::VectorStore(format!(
                "embedding dim {} != expected {}",
                emb.len(),
                EMBEDDING_DIM
            )));
        }
        flat.extend_from_slice(emb);
    }
    let values = Float32Array::from(flat);
    let item = Arc::new(Field::new("item", DataType::Float32, true));
    let vectors =
        FixedSizeListArray::try_new(item, EMBEDDING_DIM as i32, Arc::new(values), None)
            .map_err(|e| AppError::VectorStore(format!("build vector column: {e}")))?;

    RecordBatch::try_new(
        chunks_schema(),
        vec![
            Arc::new(ids),
            Arc::new(texts),
            Arc::new(sources),
            Arc::new(timestamps),
            Arc::new(vectors),
        ],
    )
    .map_err(|e| AppError::VectorStore(format!("build record batch: {e}")))
}

/// Decode search-result rows back into [`RetrievedChunk`]s.
fn rows_from_batch(batch: &RecordBatch) -> AppResult<Vec<RetrievedChunk>> {
    let texts = downcast_str(batch, "text")?;
    let sources = downcast_str(batch, "source_file")?;
    let ids = downcast_str(batch, "id")?;
    let timestamps = downcast_i64(batch, "timestamp")?;
    let distances = downcast_f32(batch, DISTANCE_COL)?;

    let mut out = Vec::with_capacity(batch.num_rows());
    for i in 0..batch.num_rows() {
        out.push(RetrievedChunk {
            text: texts.value(i).to_string(),
            metadata: ChunkMetadata {
                source_file: sources.value(i).to_string(),
                timestamp: timestamps.value(i),
                chunk_id: ids.value(i).to_string(),
            },
            score: distances.value(i),
        });
    }
    Ok(out)
}

/// Decode scan rows (no `_distance` column) into [`DocumentChunk`]s.
fn chunk_rows_from_batch(batch: &RecordBatch) -> AppResult<Vec<DocumentChunk>> {
    let texts = downcast_str(batch, "text")?;
    let sources = downcast_str(batch, "source_file")?;
    let ids = downcast_str(batch, "id")?;
    let timestamps = downcast_i64(batch, "timestamp")?;

    let mut out = Vec::with_capacity(batch.num_rows());
    for i in 0..batch.num_rows() {
        out.push(DocumentChunk {
            text: texts.value(i).to_string(),
            metadata: ChunkMetadata {
                source_file: sources.value(i).to_string(),
                timestamp: timestamps.value(i),
                chunk_id: ids.value(i).to_string(),
            },
        });
    }
    Ok(out)
}

fn column<'a>(batch: &'a RecordBatch, name: &str) -> AppResult<&'a Arc<dyn Array>> {
    batch
        .column_by_name(name)
        .ok_or_else(|| AppError::VectorStore(format!("result missing column `{name}`")))
}

fn downcast_str<'a>(batch: &'a RecordBatch, name: &str) -> AppResult<&'a StringArray> {
    column(batch, name)?
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| AppError::VectorStore(format!("column `{name}` is not Utf8")))
}

fn downcast_i64<'a>(batch: &'a RecordBatch, name: &str) -> AppResult<&'a Int64Array> {
    column(batch, name)?
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or_else(|| AppError::VectorStore(format!("column `{name}` is not Int64")))
}

fn downcast_f32<'a>(batch: &'a RecordBatch, name: &str) -> AppResult<&'a Float32Array> {
    column(batch, name)?
        .as_any()
        .downcast_ref::<Float32Array>()
        .ok_or_else(|| AppError::VectorStore(format!("column `{name}` is not Float32")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(id: &str, text: &str) -> DocumentChunk {
        DocumentChunk {
            text: text.to_string(),
            metadata: ChunkMetadata {
                source_file: "test.md".to_string(),
                timestamp: 0,
                chunk_id: id.to_string(),
            },
        }
    }

    fn unit_vec(seed: f32) -> Vec<f32> {
        let mut v = vec![0.0; EMBEDDING_DIM];
        v[0] = seed;
        v[1] = 1.0 - seed;
        v
    }

    /// Unique temp dir per test invocation so stale tables never interfere.
    fn temp_db(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("whis-{tag}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn build_batch_rejects_wrong_dim() {
        let chunks = vec![chunk("a", "x")];
        let bad = vec![vec![0.0; EMBEDDING_DIM - 1]];
        assert!(build_batch(&chunks, &bad).is_err());
    }

    #[test]
    fn build_batch_shapes_rows_and_columns() {
        let chunks = vec![chunk("a", "x"), chunk("b", "y")];
        let embs = vec![unit_vec(0.1), unit_vec(0.9)];
        let batch = build_batch(&chunks, &embs).unwrap();
        assert_eq!(batch.num_rows(), 2);
        assert_eq!(batch.num_columns(), 5);
    }

    #[tokio::test]
    async fn insert_then_search_roundtrips() {
        let dir = temp_db("store-test");
        let store = VectorStore::connect(&dir).await.unwrap();

        let chunks = vec![chunk("near", "the near one"), chunk("far", "the far one")];
        let embs = vec![unit_vec(1.0), unit_vec(0.0)];
        store.insert(&chunks, &embs).await.unwrap();

        // Query closest to the "near" vector.
        let results = store.search(&unit_vec(1.0), 2).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].metadata.chunk_id, "near");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn search_on_empty_db_returns_nothing() {
        let dir = temp_db("empty-test");
        let store = VectorStore::connect(&dir).await.unwrap();
        let results = store.search(&unit_vec(0.5), 5).await.unwrap();
        assert!(results.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Map a LanceDB error into the app error type.
fn lance_err(e: lancedb::Error) -> AppError {
    AppError::VectorStore(format!("lancedb: {e}"))
}
