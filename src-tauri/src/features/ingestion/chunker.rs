//! Text chunking shared by every parser/scraper.
//!
//! Extractors produce raw text; this module slices it into bounded pieces and
//! attaches [`ChunkMetadata`]. Splitting is paragraph-aware: paragraphs are
//! greedily packed up to [`MAX_CHARS`], and any single oversized paragraph is
//! hard-split so no chunk exceeds the bound.

use crate::shared::models::{ChunkMetadata, DocumentChunk};

/// Upper bound on characters per chunk (keeps embeddings within model context).
pub const MAX_CHARS: usize = 1200;

/// Current Unix time in seconds (0 if the clock is before the epoch).
pub fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Split text into bounded pieces on paragraph boundaries.
pub fn split_text(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();

    for para in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        // A paragraph that alone exceeds the bound is hard-split.
        if para.len() > MAX_CHARS {
            flush(&mut current, &mut out);
            out.extend(hard_split(para));
            continue;
        }
        // +2 accounts for the "\n\n" re-joiner between packed paragraphs.
        if current.len() + para.len() + 2 > MAX_CHARS {
            flush(&mut current, &mut out);
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(para);
    }
    flush(&mut current, &mut out);
    out
}

/// Attach metadata to ordered pieces, producing storable chunks. `source_file`
/// is the originating path/URL; chunk ids are `"{source_file}#{index}"`.
pub fn to_chunks(source_file: &str, pieces: Vec<String>) -> Vec<DocumentChunk> {
    let ts = now_ts();
    pieces
        .into_iter()
        .enumerate()
        .map(|(i, text)| DocumentChunk {
            text,
            metadata: ChunkMetadata {
                source_file: source_file.to_string(),
                timestamp: ts,
                chunk_id: format!("{source_file}#{i}"),
            },
        })
        .collect()
}

fn flush(current: &mut String, out: &mut Vec<String>) {
    if !current.is_empty() {
        out.push(std::mem::take(current));
    }
}

/// Hard-split an oversized string into <= MAX_CHARS windows on char boundaries.
fn hard_split(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for ch in s.chars() {
        if buf.len() + ch.len_utf8() > MAX_CHARS {
            out.push(std::mem::take(&mut buf));
        }
        buf.push(ch);
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_small_paragraphs_together() {
        let text = "one\n\ntwo\n\nthree";
        let pieces = split_text(text);
        assert_eq!(pieces, vec!["one\n\ntwo\n\nthree"]);
    }

    #[test]
    fn skips_blank_paragraphs() {
        let pieces = split_text("a\n\n\n\n   \n\nb");
        assert_eq!(pieces, vec!["a\n\nb"]);
    }

    #[test]
    fn hard_splits_oversized_paragraph() {
        let big = "x".repeat(MAX_CHARS * 2 + 10);
        let pieces = split_text(&big);
        assert!(pieces.len() >= 3);
        assert!(pieces.iter().all(|p| p.len() <= MAX_CHARS));
    }

    #[test]
    fn starts_new_chunk_when_bound_exceeded() {
        let p = "y".repeat(MAX_CHARS - 100);
        let text = format!("{p}\n\n{p}");
        let pieces = split_text(&text);
        assert_eq!(pieces.len(), 2);
    }

    #[test]
    fn to_chunks_assigns_ordered_ids() {
        let chunks = to_chunks("doc.md", vec!["a".into(), "b".into()]);
        assert_eq!(chunks[0].metadata.chunk_id, "doc.md#0");
        assert_eq!(chunks[1].metadata.chunk_id, "doc.md#1");
        assert_eq!(chunks[1].metadata.source_file, "doc.md");
    }
}
