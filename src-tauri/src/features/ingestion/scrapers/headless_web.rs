//! Headless-Chrome scraper for JavaScript-heavy pages.
//!
//! Drives a real Chromium via `chromiumoxide`, navigates to the URL, and reads
//! the rendered DOM's HTML — so client-side-rendered content is captured. The
//! HTML→text reduction ([`html_to_text`]) is pure and unit-tested; the browser
//! path requires a Chrome binary at runtime and is exercised live.

use std::sync::OnceLock;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::error::CdpError;
use futures_util::StreamExt;
use regex::Regex;

use crate::shared::error::{AppError, AppResult};

/// Render `url` in headless Chrome and return its extracted visible text.
pub async fn scrape_url(url: &str) -> AppResult<String> {
    let config = BrowserConfig::builder()
        .build()
        .map_err(|e| AppError::Ingestion(format!("browser config: {e}")))?;

    let (mut browser, mut handler) = Browser::launch(config).await.map_err(cdp_err)?;

    // The handler future must be polled for the browser to make progress.
    let pump = tokio::spawn(async move {
        while let Some(event) = handler.next().await {
            if event.is_err() {
                break;
            }
        }
    });

    // Run navigation in an inner scope so we always tear the browser down after.
    let result = async {
        let page = browser.new_page(url).await.map_err(cdp_err)?;
        page.wait_for_navigation().await.map_err(cdp_err)?;
        let html = page.content().await.map_err(cdp_err)?;
        Ok::<String, AppError>(html_to_text(&html))
    }
    .await;

    let _ = browser.close().await;
    pump.abort();
    result
}

/// Reduce an HTML document to readable plain text: drop `<script>`/`<style>`
/// blocks, strip tags, decode common entities, and collapse whitespace.
pub fn html_to_text(html: &str) -> String {
    let without_blocks = block_regex().replace_all(html, " ");
    let without_tags = tag_regex().replace_all(&without_blocks, " ");
    let decoded = decode_entities(&without_tags);
    collapse_whitespace(&decoded)
}

fn block_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?is)<(script|style)\b[^>]*>.*?</\s*(script|style)\s*>")
            .expect("valid block regex")
    })
}

fn tag_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<[^>]+>").expect("valid tag regex"))
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn cdp_err(e: CdpError) -> AppError {
    AppError::Ingestion(format!("headless browser: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_collapses_whitespace() {
        let html = "<html><body>  <p>Hello</p>\n<p>World</p> </body></html>";
        assert_eq!(html_to_text(html), "Hello World");
    }

    #[test]
    fn removes_script_and_style_blocks() {
        let html = "<style>.x{color:red}</style><p>keep</p><script>alert(1)</script>";
        assert_eq!(html_to_text(html), "keep");
    }

    #[test]
    fn decodes_common_entities() {
        let html = "<p>Tom &amp; Jerry &lt;3 &#39;quotes&#39;</p>";
        assert_eq!(html_to_text(html), "Tom & Jerry <3 'quotes'");
    }

    #[test]
    fn empty_html_is_empty() {
        assert_eq!(html_to_text(""), "");
        assert_eq!(html_to_text("<div></div>"), "");
    }
}
