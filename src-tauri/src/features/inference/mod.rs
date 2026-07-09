//! Local inference subsystem.
//!
//! Public surface: the [`HiggsClient`] (streaming + structured completions) and
//! the [`extract_cards`] helper that constrains the model to emit strict Anki
//! card JSON.

pub mod higgs_client;

use serde_json::json;

pub use higgs_client::{GenerationParams, HiggsClient};

use crate::shared::error::AppResult;
use crate::shared::models::{Card, ChatMessage};

/// JSON Schema constraining structured card generation to `{ front, back, tags }`.
fn cards_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "cards": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "front": { "type": "string" },
                        "back":  { "type": "string" },
                        "tags":  { "type": "array", "items": { "type": "string" } }
                    },
                    "required": ["front", "back", "tags"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["cards"],
        "additionalProperties": false
    })
}

/// Wrapper matching the `cards_schema` root object.
#[derive(serde::Deserialize)]
struct CardsEnvelope {
    cards: Vec<Card>,
}

/// Generate Anki cards from `messages` using strict JSON-schema output.
pub async fn extract_cards(
    client: &HiggsClient,
    messages: Vec<ChatMessage>,
    params: GenerationParams,
) -> AppResult<Vec<Card>> {
    let value = client
        .complete_structured(messages, params, "anki_cards", cards_schema())
        .await?;
    let envelope: CardsEnvelope = serde_json::from_value(value)?;
    Ok(envelope.cards)
}
