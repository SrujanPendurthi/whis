//! Ingestion subsystem: turn local files and remote sources into embeddable
//! [`DocumentChunk`]s.
//!
//! Extractors (`parsers`, `scrapers`) produce plain text; [`chunker`] slices it
//! and attaches metadata. Public entry points dispatch by file extension or
//! source type and return ready-to-index chunks.

pub mod chunker;
pub mod parsers;
pub mod scrapers;
pub(crate) mod xml;

use std::path::Path;

use crate::shared::error::{AppError, AppResult};
use crate::shared::models::DocumentChunk;

/// Supported local document formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Markdown,
    Pdf,
    Pptx,
    PlainText,
}

/// Map a lowercase file extension to a [`SourceKind`], if supported.
pub fn classify(ext: &str) -> Option<SourceKind> {
    match ext.to_ascii_lowercase().as_str() {
        "md" | "markdown" => Some(SourceKind::Markdown),
        "pdf" => Some(SourceKind::Pdf),
        "pptx" => Some(SourceKind::Pptx),
        "txt" | "text" => Some(SourceKind::PlainText),
        _ => None,
    }
}

/// Ingest a local file, returning embeddable chunks. Dispatches on extension.
pub async fn ingest_path(path: impl AsRef<Path>) -> AppResult<Vec<DocumentChunk>> {
    let path = path.as_ref();
    let source = path.to_string_lossy().to_string();
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| AppError::Ingestion(format!("no file extension: {source}")))?;
    let kind = classify(ext)
        .ok_or_else(|| AppError::Ingestion(format!("unsupported file type: .{ext}")))?;

    let pieces = match kind {
        SourceKind::Markdown => {
            let text = tokio::fs::read_to_string(path).await?;
            parsers::markdown::split_sections(&text)
                .iter()
                .flat_map(|s| chunker::split_text(s))
                .collect()
        }
        SourceKind::PlainText => {
            let text = tokio::fs::read_to_string(path).await?;
            chunker::split_text(&text)
        }
        SourceKind::Pdf => {
            let bytes = tokio::fs::read(path).await?;
            chunker::split_text(&parsers::pdf::extract_text(&bytes)?)
        }
        SourceKind::Pptx => {
            let bytes = tokio::fs::read(path).await?;
            chunker::split_text(&parsers::pptx::extract_text(&bytes)?)
        }
    };

    Ok(chunker::to_chunks(&source, pieces))
}

/// Scrape a web page (headless Chrome) into chunks tagged with the URL.
pub async fn ingest_web(url: &str) -> AppResult<Vec<DocumentChunk>> {
    let text = scrapers::headless_web::scrape_url(url).await?;
    Ok(chunker::to_chunks(url, chunker::split_text(&text)))
}

/// Fetch a YouTube transcript into chunks tagged with the source URL.
pub async fn ingest_youtube(url: &str) -> AppResult<Vec<DocumentChunk>> {
    let id = scrapers::youtube_transcript::extract_video_id(url)
        .ok_or_else(|| AppError::Ingestion(format!("not a YouTube URL: {url}")))?;
    let text = scrapers::youtube_transcript::fetch_transcript(&id).await?;
    Ok(chunker::to_chunks(url, chunker::split_text(&text)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_supported_extensions() {
        assert_eq!(classify("md"), Some(SourceKind::Markdown));
        assert_eq!(classify("MARKDOWN"), Some(SourceKind::Markdown));
        assert_eq!(classify("pdf"), Some(SourceKind::Pdf));
        assert_eq!(classify("pptx"), Some(SourceKind::Pptx));
        assert_eq!(classify("txt"), Some(SourceKind::PlainText));
        assert_eq!(classify("docx"), None);
    }

    #[tokio::test]
    async fn ingests_a_markdown_file() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("whis-ingest-{}.md", std::process::id()));
        tokio::fs::write(&path, "# Title\nintro\n\n## Section\nbody text")
            .await
            .unwrap();

        let chunks = ingest_path(&path).await.unwrap();
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].text.starts_with("# Title"));
        assert!(chunks[0].metadata.source_file.ends_with(".md"));
        assert_eq!(chunks[0].metadata.chunk_id, format!("{}#0", path.to_string_lossy()));

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn rejects_unsupported_extension() {
        let err = ingest_path("/tmp/whatever.docx").await;
        assert!(err.is_err());
    }
}
