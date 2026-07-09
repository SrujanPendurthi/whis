//! Async client for the local AnkiConnect endpoint (`127.0.0.1:8765`).
//!
//! AnkiConnect speaks a uniform JSON-RPC-ish protocol: every request is
//! `{ action, version: 6, params }` and every response is `{ result, error }`.
//! Request assembly and response unwrapping are pure functions so the protocol
//! is unit-tested without a running Anki. Connection failures degrade
//! gracefully — [`AnkiClient::can_connect`] never errors — so the desktop app
//! thread is never brought down by Anki being closed.

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::shared::config::ANKI_CONNECT_URL;
use crate::shared::error::{AppError, AppResult};
use crate::shared::models::Card;

/// AnkiConnect protocol version this client targets.
const ANKI_API_VERSION: u32 = 6;
/// Note type used for generated cards.
const MODEL_NAME: &str = "Basic";

/// Async client for AnkiConnect.
#[derive(Debug, Clone)]
pub struct AnkiClient {
    http: reqwest::Client,
    url: String,
}

impl Default for AnkiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AnkiClient {
    /// Client pointed at the default local AnkiConnect endpoint.
    pub fn new() -> Self {
        Self::with_url(ANKI_CONNECT_URL)
    }

    /// Client pointed at an arbitrary endpoint (used by tests).
    pub fn with_url(url: impl Into<String>) -> Self {
        Self { http: reqwest::Client::new(), url: url.into() }
    }

    /// Invoke an AnkiConnect action and deserialize its `result` field.
    async fn invoke<T: DeserializeOwned>(&self, action: &str, params: Value) -> AppResult<T> {
        let body = build_request(action, params);
        let value: Value = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Anki(format!("request failed: {e}")))?
            .json()
            .await
            .map_err(|e| AppError::Anki(format!("invalid response: {e}")))?;
        parse_response(value)
    }

    /// True iff Anki + AnkiConnect are reachable. Never errors.
    pub async fn can_connect(&self) -> bool {
        self.invoke::<i64>("version", json!({})).await.is_ok()
    }

    /// List existing deck names.
    pub async fn deck_names(&self) -> AppResult<Vec<String>> {
        self.invoke("deckNames", json!({})).await
    }

    /// Create `deck` if it does not already exist (idempotent).
    pub async fn create_deck_if_missing(&self, deck: &str) -> AppResult<()> {
        let names = self.deck_names().await?;
        if !names.iter().any(|n| n == deck) {
            let _: Value = self.invoke("createDeck", json!({ "deck": deck })).await?;
        }
        Ok(())
    }

    /// Add `cards` to `deck`. Returns the per-card note ids (`None` where Anki
    /// rejected the note, e.g. a duplicate).
    pub async fn add_notes(&self, deck: &str, cards: &[Card]) -> AppResult<Vec<Option<i64>>> {
        let notes: Vec<Value> = cards.iter().map(|c| note_from_card(deck, c)).collect();
        self.invoke("addNotes", json!({ "notes": notes })).await
    }

    /// Ensure the deck exists, then add the cards. The high-level entry point.
    pub async fn sync_cards(&self, deck: &str, cards: &[Card]) -> AppResult<Vec<Option<i64>>> {
        self.create_deck_if_missing(deck).await?;
        self.add_notes(deck, cards).await
    }
}

/// Assemble an AnkiConnect request envelope.
fn build_request(action: &str, params: Value) -> Value {
    json!({
        "action": action,
        "version": ANKI_API_VERSION,
        "params": params,
    })
}

/// Convert a [`Card`] into an AnkiConnect note payload.
fn note_from_card(deck: &str, card: &Card) -> Value {
    json!({
        "deckName": deck,
        "modelName": MODEL_NAME,
        "fields": { "Front": card.front, "Back": card.back },
        "tags": card.tags,
        "options": { "allowDuplicate": false },
    })
}

/// Unwrap an AnkiConnect `{ result, error }` response into `T`. A non-empty
/// `error` becomes an [`AppError::Anki`]; otherwise `result` is deserialized.
fn parse_response<T: DeserializeOwned>(value: Value) -> AppResult<T> {
    if let Some(err) = value.get("error").and_then(|e| e.as_str()).filter(|s| !s.is_empty()) {
        return Err(AppError::Anki(err.to_string()));
    }
    let result = value.get("result").cloned().unwrap_or(Value::Null);
    serde_json::from_value(result).map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> Card {
        Card {
            front: "What is 2+2?".into(),
            back: "4".into(),
            tags: vec!["math".into(), "arithmetic".into()],
        }
    }

    #[test]
    fn request_envelope_shape() {
        let req = build_request("deckNames", json!({}));
        assert_eq!(req["action"], json!("deckNames"));
        assert_eq!(req["version"], json!(6));
        assert_eq!(req["params"], json!({}));
    }

    #[test]
    fn note_payload_shape() {
        let note = note_from_card("Study::Math", &card());
        assert_eq!(note["deckName"], json!("Study::Math"));
        assert_eq!(note["modelName"], json!("Basic"));
        assert_eq!(note["fields"]["Front"], json!("What is 2+2?"));
        assert_eq!(note["fields"]["Back"], json!("4"));
        assert_eq!(note["tags"], json!(["math", "arithmetic"]));
        assert_eq!(note["options"]["allowDuplicate"], json!(false));
    }

    #[test]
    fn parse_response_returns_result() {
        let v = json!({ "result": ["Default", "Study"], "error": null });
        let decks: Vec<String> = parse_response(v).unwrap();
        assert_eq!(decks, vec!["Default", "Study"]);
    }

    #[test]
    fn parse_response_surfaces_error() {
        let v = json!({ "result": null, "error": "deck not found" });
        let out: AppResult<Vec<String>> = parse_response(v);
        assert!(matches!(out, Err(AppError::Anki(m)) if m == "deck not found"));
    }

    #[test]
    fn parse_response_handles_addnotes_nulls() {
        // addNotes returns note ids, with null for rejected (duplicate) notes.
        let v = json!({ "result": [1502298033753i64, null], "error": null });
        let ids: Vec<Option<i64>> = parse_response(v).unwrap();
        assert_eq!(ids, vec![Some(1502298033753i64), None]);
    }

    #[test]
    fn empty_error_string_is_not_an_error() {
        let v = json!({ "result": 6, "error": "" });
        let version: i64 = parse_response(v).unwrap();
        assert_eq!(version, 6);
    }
}
