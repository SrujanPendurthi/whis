//! YouTube transcript scraper.
//!
//! Extracts the 11-char video id from any common URL form, then fetches the
//! caption track from `youtube.com/api/timedtext` and parses the `<text>` XML.
//! ID extraction and XML parsing are pure and unit-tested; only [`fetch_transcript`]
//! touches the network (best-effort: the endpoint returns empty for videos with
//! no published English captions).

use std::sync::OnceLock;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use regex::Regex;

use crate::shared::error::{AppError, AppResult};

/// Compiled once: matches the id in `v=`, `youtu.be/`, `/embed/`, `/shorts/`,
/// `/v/` forms.
fn id_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?:v=|youtu\.be/|/embed/|/shorts/|/v/)([0-9A-Za-z_-]{11})")
            .expect("valid youtube id regex")
    })
}

/// Extract a video id from a URL or accept a bare 11-char id.
pub fn extract_video_id(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if is_bare_id(trimmed) {
        return Some(trimmed.to_string());
    }
    id_regex()
        .captures(trimmed)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn is_bare_id(s: &str) -> bool {
    s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Fetch and parse the English caption track for `video_id`.
pub async fn fetch_transcript(video_id: &str) -> AppResult<String> {
    let url = format!("https://www.youtube.com/api/timedtext?lang=en&v={video_id}");
    let xml = reqwest::get(&url)
        .await
        .map_err(|e| AppError::Ingestion(format!("timedtext fetch: {e}")))?
        .text()
        .await
        .map_err(|e| AppError::Ingestion(format!("timedtext body: {e}")))?;
    Ok(parse_timedtext(&xml))
}

/// Parse timedtext XML into newline-joined caption lines.
pub fn parse_timedtext(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    let mut lines: Vec<String> = Vec::new();
    let mut in_text = false;
    let mut current = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"text" => {
                in_text = true;
                current.clear();
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"text" => {
                in_text = false;
                let line = current.trim();
                if !line.is_empty() {
                    lines.push(line.to_string());
                }
            }
            Ok(Event::Text(e)) if in_text => {
                if let Ok(decoded) = e.decode() {
                    current.push_str(&decoded);
                }
            }
            // quick-xml emits entity refs (`&amp;`, `&#39;`) as their own events.
            Ok(Event::GeneralRef(e)) if in_text => {
                if let Some(c) = crate::features::ingestion::xml::resolve_entity(&e) {
                    current.push(c);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_id_from_watch_url() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=5s").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn extracts_id_from_short_and_embed_forms() {
        assert_eq!(extract_video_id("https://youtu.be/dQw4w9WgXcQ").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(
            extract_video_id("https://www.youtube.com/embed/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
        assert_eq!(
            extract_video_id("https://www.youtube.com/shorts/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn accepts_bare_id_and_rejects_junk() {
        assert_eq!(extract_video_id("dQw4w9WgXcQ").as_deref(), Some("dQw4w9WgXcQ"));
        assert_eq!(extract_video_id("not a url"), None);
        assert_eq!(extract_video_id("https://example.com/watch?v=short"), None);
    }

    #[test]
    fn parses_caption_lines() {
        let xml = r#"<transcript><text start="0" dur="1">Hello there</text>
            <text start="1" dur="1">General &amp; Kenobi</text></transcript>"#;
        assert_eq!(parse_timedtext(xml), "Hello there\nGeneral & Kenobi");
    }

    #[test]
    fn empty_transcript_is_empty() {
        assert_eq!(parse_timedtext("<transcript></transcript>"), "");
    }
}
