//! App-wide settings, endpoint constants, and path locators.
//!
//! The LanceDB directory is intentionally NOT hardcoded anywhere that opens a
//! connection — [`default_db_path`] only supplies a fallback used when a caller
//! provides no explicit path. See `features/vector_db/store.rs`, whose connect
//! API always takes the path as a parameter.

use std::path::PathBuf;

use crate::shared::error::{AppError, AppResult};

/// Local Higgs (MLX) inference server, OpenAI/Anthropic-compatible `/v1` API.
pub const HIGGS_BASE_URL: &str = "http://127.0.0.1:8080/v1";

/// Local AnkiConnect endpoint exposed by the Anki desktop plugin.
pub const ANKI_CONNECT_URL: &str = "http://127.0.0.1:8765";

/// Embedding model identifier (runs locally via candle on Metal).
pub const EMBEDDING_MODEL_ID: &str = "BAAI/bge-small-en-v1.5";

/// Dimensionality of `bge-small-en-v1.5` embeddings.
pub const EMBEDDING_DIM: usize = 384;

/// Number of nearest chunks returned by a similarity search.
pub const TOP_K: usize = 5;

/// Application data root: `~/.local/share/whis` (override with `WHIS_DATA_DIR`).
///
/// This is the app's own data directory; it is distinct from the LanceDB path,
/// which callers may point anywhere via the store API.
pub fn data_dir() -> AppResult<PathBuf> {
    if let Ok(dir) = std::env::var("WHIS_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let home = std::env::var("HOME")
        .map_err(|_| AppError::Config("HOME environment variable is not set".into()))?;
    Ok(PathBuf::from(home).join(".local/share/whis"))
}

/// Fallback LanceDB directory (`<data_dir>/db`) used only when a caller does not
/// specify one. The store never hardcodes this — it is passed in explicitly.
pub fn default_db_path() -> AppResult<PathBuf> {
    Ok(data_dir()?.join("db"))
}
