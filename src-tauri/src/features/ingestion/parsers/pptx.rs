//! PowerPoint (.pptx) text extraction.
//!
//! A `.pptx` is a zip archive; slide text lives in `ppt/slides/slideN.xml` as
//! `<a:t>` runs grouped by `<a:p>` paragraphs. We read the slides in order and
//! pull the text out of each. The XML parsing ([`parse_slide_text`]) is pure so
//! it is unit-tested without building a zip.

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::reader::Reader;

use crate::shared::error::{AppError, AppResult};

/// Extract all slide text from an in-memory `.pptx`, slides in order, separated
/// by blank lines.
pub fn extract_text(bytes: &[u8]) -> AppResult<String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| AppError::Ingestion(format!("pptx open failed: {e}")))?;

    // Collect (slide_number, xml) so we can emit slides in presentation order.
    let mut slides: Vec<(u32, String)> = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AppError::Ingestion(format!("pptx entry read failed: {e}")))?;
        let name = file.name().to_string();
        if let Some(num) = slide_number(&name) {
            let mut xml = String::new();
            file.read_to_string(&mut xml)
                .map_err(|e| AppError::Ingestion(format!("pptx slide read failed: {e}")))?;
            slides.push((num, xml));
        }
    }
    slides.sort_by_key(|(n, _)| *n);

    let text = slides
        .iter()
        .map(|(_, xml)| parse_slide_text(xml))
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(text.trim().to_string())
}

/// Parse `slideN.xml` from a `ppt/slides/` path, returning its slide index.
fn slide_number(entry_name: &str) -> Option<u32> {
    let file = entry_name.strip_prefix("ppt/slides/slide")?;
    let stem = file.strip_suffix(".xml")?;
    stem.parse::<u32>().ok()
}

/// Extract the visible text from one slide's XML. `<a:t>` runs are concatenated;
/// each `<a:p>` paragraph is emitted on its own line.
pub fn parse_slide_text(xml: &str) -> String {
    let mut reader = Reader::from_str(xml);
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut in_text_run = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"a:t" => in_text_run = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"a:t" => in_text_run = false,
            Ok(Event::Text(e)) if in_text_run => {
                if let Ok(decoded) = e.decode() {
                    line.push_str(&decoded);
                }
            }
            // quick-xml emits entity refs (`&amp;`, `&#39;`) as their own events.
            Ok(Event::GeneralRef(e)) if in_text_run => {
                if let Some(c) = crate::features::ingestion::xml::resolve_entity(&e) {
                    line.push(c);
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"a:p" => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
                line.clear();
            }
            Ok(Event::Eof) => break,
            Err(_) => break, // best-effort: stop on malformed XML
            _ => {}
        }
    }
    if !line.trim().is_empty() {
        lines.push(line.trim().to_string());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_slide_paths() {
        assert_eq!(slide_number("ppt/slides/slide1.xml"), Some(1));
        assert_eq!(slide_number("ppt/slides/slide12.xml"), Some(12));
        assert_eq!(slide_number("ppt/slides/_rels/slide1.xml.rels"), None);
        assert_eq!(slide_number("ppt/theme/theme1.xml"), None);
    }

    #[test]
    fn extracts_runs_and_paragraphs() {
        let xml = r#"<p:sld><a:p><a:r><a:t>Hello </a:t></a:r><a:r><a:t>World</a:t></a:r></a:p>
            <a:p><a:r><a:t>Second line</a:t></a:r></a:p></p:sld>"#;
        assert_eq!(parse_slide_text(xml), "Hello World\nSecond line");
    }

    #[test]
    fn unescapes_entities() {
        let xml = r#"<a:p><a:t>A &amp; B &lt;tag&gt;</a:t></a:p>"#;
        assert_eq!(parse_slide_text(xml), "A & B <tag>");
    }

    #[test]
    fn empty_slide_yields_empty_string() {
        assert_eq!(parse_slide_text("<p:sld></p:sld>"), "");
    }
}
