//! PDF text extraction via the `pdf-extract` crate.
//!
//! Extraction is best-effort: scanned/image-only PDFs yield little or no text.
//! Operates on an in-memory buffer so callers can pass bytes from any source.

use crate::shared::error::{AppError, AppResult};

/// Extract the plain-text content of a PDF held in memory.
pub fn extract_text(bytes: &[u8]) -> AppResult<String> {
    pdf_extract::extract_text_from_mem(bytes)
        .map(|t| t.trim().to_string())
        .map_err(|e| AppError::Ingestion(format!("pdf extraction failed: {e}")))
}
