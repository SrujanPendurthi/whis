//! HTTP client for the local Higgs (MLX) inference server.
//!
//! Targets an OpenAI-compatible `/v1/chat/completions` endpoint on
//! `127.0.0.1:8080`. Two entry points:
//!   * [`HiggsClient::stream_chat`] — streams token deltas over a
//!     `tokio::sync::mpsc` channel (SSE, `stream: true`).
//!   * [`HiggsClient::complete_structured`] — a single non-streamed call that
//!     constrains output to a JSON Schema (used for strict card extraction).
//!
//! The SSE decoding is factored into pure functions ([`parse_sse_line`]) so the
//! wire protocol is unit-tested without needing a live server.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::shared::config::HIGGS_BASE_URL;
use crate::shared::error::{AppError, AppResult};
use crate::shared::models::ChatMessage;

/// Default model served by the local engine. Overridable per-request.
const DEFAULT_MODEL: &str = "local";

/// Buffered token channel depth. Keeps the producer from racing far ahead of a
/// slow UI consumer without allocating unboundedly.
const CHANNEL_CAPACITY: usize = 64;

/// Tunable generation parameters for a chat request.
#[derive(Debug, Clone)]
pub struct GenerationParams {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: Option<u32>,
}

impl Default for GenerationParams {
    fn default() -> Self {
        Self { model: DEFAULT_MODEL.to_string(), temperature: 0.7, max_tokens: None }
    }
}

/// Async client for the local inference server.
#[derive(Debug, Clone)]
pub struct HiggsClient {
    http: reqwest::Client,
    base_url: String,
}

impl Default for HiggsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HiggsClient {
    /// Client pointed at the default local endpoint (`127.0.0.1:8080/v1`).
    pub fn new() -> Self {
        Self::with_base_url(HIGGS_BASE_URL)
    }

    /// Client pointed at an arbitrary base URL (used by tests / alt ports).
    pub fn with_base_url(base_url: impl Into<String>) -> Self {
        Self { http: reqwest::Client::new(), base_url: base_url.into() }
    }

    fn chat_completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    /// Stream a chat completion. Returns immediately with a receiver; a spawned
    /// task drives the HTTP request and forwards each content delta as
    /// `Ok(token)`. A transport/parse failure is delivered as a final `Err` and
    /// then the channel closes. Normal completion simply closes the channel.
    pub async fn stream_chat(
        &self,
        messages: Vec<ChatMessage>,
        params: GenerationParams,
    ) -> AppResult<mpsc::Receiver<AppResult<String>>> {
        let body = build_request_body(&messages, &params, true, None);
        let resp = self
            .http
            .post(self.chat_completions_url())
            .json(&body)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| AppError::Inference(e.to_string()))?;

        let (tx, rx) = mpsc::channel::<AppResult<String>>(CHANNEL_CAPACITY);
        tokio::spawn(async move {
            if let Err(e) = pump_sse(resp, &tx).await {
                // Best-effort delivery of the terminal error; ignore if the
                // consumer has already hung up.
                let _ = tx.send(Err(e)).await;
            }
        });
        Ok(rx)
    }

    /// Non-streaming completion constrained to `schema`. Returns the parsed JSON
    /// value the model produced (guaranteed to match the schema when the engine
    /// honors `response_format`). `schema_name` labels the schema for the API.
    pub async fn complete_structured(
        &self,
        messages: Vec<ChatMessage>,
        params: GenerationParams,
        schema_name: &str,
        schema: Value,
    ) -> AppResult<Value> {
        let response_format = json!({
            "type": "json_schema",
            "json_schema": { "name": schema_name, "schema": schema, "strict": true }
        });
        let body = build_request_body(&messages, &params, false, Some(response_format));

        let resp = self
            .http
            .post(self.chat_completions_url())
            .json(&body)
            .send()
            .await?
            .error_for_status()
            .map_err(|e| AppError::Inference(e.to_string()))?;

        let parsed: ChatCompletionResponse = resp.json().await?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message)
            .map(|m| m.content)
            .ok_or_else(|| AppError::Inference("engine returned no choices".into()))?;

        serde_json::from_str(&content).map_err(AppError::from)
    }
}

/// Assemble the JSON request body for `/chat/completions`.
fn build_request_body(
    messages: &[ChatMessage],
    params: &GenerationParams,
    stream: bool,
    response_format: Option<Value>,
) -> Value {
    let mut body = json!({
        "model": params.model,
        "temperature": params.temperature,
        "stream": stream,
        "messages": messages,
    });
    if let Some(max) = params.max_tokens {
        body["max_tokens"] = json!(max);
    }
    if let Some(rf) = response_format {
        body["response_format"] = rf;
    }
    body
}

/// Read the SSE byte stream, decode each `data:` line, and forward content
/// deltas over `tx`. Returns `Ok(())` on a clean `[DONE]` / stream end.
async fn pump_sse(
    resp: reqwest::Response,
    tx: &mpsc::Sender<AppResult<String>>,
) -> AppResult<()> {
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        buf.push_str(&String::from_utf8_lossy(&bytes));

        // Emit every complete line currently in the buffer.
        while let Some(nl) = buf.find('\n') {
            let line: String = buf.drain(..=nl).collect();
            match parse_sse_line(line.trim_end())? {
                SseLine::Skip => {}
                SseLine::Done => return Ok(()),
                SseLine::Content(text) => {
                    if !text.is_empty() && tx.send(Ok(text)).await.is_err() {
                        // Consumer dropped; stop pulling from the network.
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

/// Classification of a single decoded SSE line.
#[derive(Debug, PartialEq, Eq)]
enum SseLine {
    /// Comment, blank line, or a non-`data:` field — ignore.
    Skip,
    /// Terminal `data: [DONE]` sentinel.
    Done,
    /// A content delta extracted from the chunk (may be empty).
    Content(String),
}

/// Decode one already-trimmed SSE line into an [`SseLine`].
///
/// Pure and side-effect free so the wire format is unit-testable.
fn parse_sse_line(line: &str) -> AppResult<SseLine> {
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(SseLine::Skip);
    };
    let data = data.trim();
    if data.is_empty() {
        return Ok(SseLine::Skip);
    }
    if data == "[DONE]" {
        return Ok(SseLine::Done);
    }

    let chunk: ChatCompletionChunk = serde_json::from_str(data)?;
    let content = chunk
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.delta.content)
        .unwrap_or_default();
    Ok(SseLine::Content(content))
}

// --- Wire types (OpenAI-compatible subset) ---------------------------------

#[derive(Debug, Deserialize)]
struct ChatCompletionChunk {
    #[serde(default)]
    choices: Vec<ChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChunkChoice {
    delta: Delta,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    #[serde(default)]
    choices: Vec<ResponseChoice>,
}

#[derive(Debug, Deserialize)]
struct ResponseChoice {
    message: Option<ResponseMessage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ResponseMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_blank_and_comment_lines() {
        assert_eq!(parse_sse_line("").unwrap(), SseLine::Skip);
        assert_eq!(parse_sse_line(": keep-alive").unwrap(), SseLine::Skip);
        assert_eq!(parse_sse_line("event: message").unwrap(), SseLine::Skip);
        assert_eq!(parse_sse_line("data:").unwrap(), SseLine::Skip);
        assert_eq!(parse_sse_line("data:   ").unwrap(), SseLine::Skip);
    }

    #[test]
    fn detects_done_sentinel() {
        assert_eq!(parse_sse_line("data: [DONE]").unwrap(), SseLine::Done);
        assert_eq!(parse_sse_line("data:[DONE]").unwrap(), SseLine::Done);
    }

    #[test]
    fn extracts_content_delta() {
        let line = r#"data: {"choices":[{"delta":{"content":"Hel"}}]}"#;
        assert_eq!(parse_sse_line(line).unwrap(), SseLine::Content("Hel".into()));
    }

    #[test]
    fn role_only_delta_yields_empty_content() {
        let line = r#"data: {"choices":[{"delta":{"role":"assistant"}}]}"#;
        assert_eq!(parse_sse_line(line).unwrap(), SseLine::Content(String::new()));
    }

    #[test]
    fn empty_choices_yields_empty_content() {
        let line = r#"data: {"choices":[]}"#;
        assert_eq!(parse_sse_line(line).unwrap(), SseLine::Content(String::new()));
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_sse_line("data: {not json}").is_err());
    }

    #[test]
    fn request_body_shapes_correctly() {
        let msgs = vec![ChatMessage::user("hi")];
        let params = GenerationParams { max_tokens: Some(128), ..Default::default() };
        let body = build_request_body(&msgs, &params, true, None);
        assert_eq!(body["stream"], json!(true));
        assert_eq!(body["max_tokens"], json!(128));
        assert_eq!(body["messages"][0]["role"], json!("user"));
        assert_eq!(body["messages"][0]["content"], json!("hi"));
        assert!(body.get("response_format").is_none());
    }

    #[test]
    fn structured_body_includes_response_format() {
        let msgs = vec![ChatMessage::user("cards")];
        let rf = json!({"type": "json_schema"});
        let body = build_request_body(&msgs, &GenerationParams::default(), false, Some(rf));
        assert_eq!(body["stream"], json!(false));
        assert_eq!(body["response_format"]["type"], json!("json_schema"));
    }
}
