//! Core shared types and IPC payload structs.
//!
//! These types cross the Rust <-> TypeScript boundary. Every struct uses
//! `camelCase` on the wire so the matching TS interfaces read idiomatically.
//! Keep the TS mirror in `src/shared/types.ts` in sync with this file.

use serde::{Deserialize, Serialize};

/// Author of a chat turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

/// A single conversation turn sent to / received from the inference engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: Role::System, content: content.into() }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

/// Metadata attached to every stored chunk in the vector DB.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkMetadata {
    /// Absolute path or URL the chunk was derived from.
    pub source_file: String,
    /// Unix epoch seconds when the chunk was ingested.
    pub timestamp: i64,
    /// Stable per-chunk identifier (unique within a source).
    pub chunk_id: String,
}

/// A text chunk ready to be embedded and inserted into the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub text: String,
    #[serde(flatten)]
    pub metadata: ChunkMetadata,
}

/// A chunk returned from a similarity search, with its distance score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub text: String,
    #[serde(flatten)]
    pub metadata: ChunkMetadata,
    /// Similarity distance (lower is closer for L2).
    pub score: f32,
}

/// A distinct ingested source and how many chunks it contributed. Powers the
/// workspace sidebar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub source_file: String,
    pub chunk_count: usize,
}

/// A flashcard extracted for Anki. Mirrors the strict JSON schema the model is
/// constrained to when generating cards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub front: String,
    pub back: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A configured inference target the user can switch between. Every provider is
/// treated as OpenAI-compatible: a base URL, a model name, and an optional
/// bearer API key (empty for a local server that needs no auth).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProfile {
    /// Stable identifier (slug/uuid); referenced by `active_profile_id`.
    pub id: String,
    /// Human-readable name shown in the switcher (e.g. "GPT-4o", "Local Higgs").
    pub label: String,
    /// OpenAI-compatible base, incl. the `/v1` suffix (e.g. `https://api.openai.com/v1`).
    pub base_url: String,
    /// Model name sent in the request body (e.g. `gpt-4o`, `local`).
    pub model: String,
    /// Bearer token. Empty means "send no Authorization header" (local servers).
    #[serde(default)]
    pub api_key: String,
    /// Sampling temperature used for this profile.
    pub temperature: f32,
}

/// Persisted application settings. Currently just the model profiles and which
/// one is active; serialized to `data_dir()/settings.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub profiles: Vec<ModelProfile>,
    /// Id of the active profile. When `None` (or dangling), the first profile is
    /// treated as active.
    #[serde(default)]
    pub active_profile_id: Option<String>,
}

impl Default for AppSettings {
    /// Seed a single local profile pointing at the default Higgs endpoint and
    /// mark it active, so a fresh install behaves exactly as before this feature.
    fn default() -> Self {
        let local = ModelProfile {
            id: "local".to_string(),
            label: "Local (Higgs)".to_string(),
            base_url: crate::shared::config::HIGGS_BASE_URL.to_string(),
            model: "local".to_string(),
            api_key: String::new(),
            temperature: 0.7,
        };
        Self { active_profile_id: Some(local.id.clone()), profiles: vec![local] }
    }
}

impl AppSettings {
    /// Insert `profile`, or replace an existing one with the same id.
    pub fn upsert(&mut self, profile: ModelProfile) {
        match self.profiles.iter_mut().find(|p| p.id == profile.id) {
            Some(existing) => *existing = profile,
            None => self.profiles.push(profile),
        }
    }

    /// Remove the profile with `id`. If it was active, the active pointer is
    /// cleared (so `active_profile` falls back to the first remaining profile).
    pub fn remove(&mut self, id: &str) {
        self.profiles.retain(|p| p.id != id);
        if self.active_profile_id.as_deref() == Some(id) {
            self.active_profile_id = None;
        }
    }

    /// The active profile: the one whose id matches `active_profile_id`, else
    /// the first profile. `None` only when there are no profiles at all.
    pub fn active_profile(&self) -> Option<&ModelProfile> {
        self.active_profile_id
            .as_deref()
            .and_then(|id| self.profiles.iter().find(|p| p.id == id))
            .or_else(|| self.profiles.first())
    }
}

/// Incremental event streamed to the frontend as the model produces output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum StreamEvent {
    /// A token / text delta appended to the current assistant message.
    Token { text: String },
    /// A human-readable progress log line (used by `/deep-research`).
    Log { line: String },
    /// The stream finished successfully.
    Done,
    /// The stream terminated with an error message.
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str) -> ModelProfile {
        ModelProfile {
            id: id.to_string(),
            label: id.to_string(),
            base_url: "http://x/v1".to_string(),
            model: "m".to_string(),
            api_key: String::new(),
            temperature: 0.5,
        }
    }

    #[test]
    fn default_seeds_active_local_profile() {
        let s = AppSettings::default();
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.active_profile_id.as_deref(), Some("local"));
        assert_eq!(s.active_profile().unwrap().id, "local");
    }

    #[test]
    fn upsert_inserts_then_replaces() {
        let mut s = AppSettings { profiles: vec![], active_profile_id: None };
        s.upsert(profile("a"));
        assert_eq!(s.profiles.len(), 1);
        let mut updated = profile("a");
        updated.label = "renamed".into();
        s.upsert(updated);
        assert_eq!(s.profiles.len(), 1);
        assert_eq!(s.profiles[0].label, "renamed");
    }

    #[test]
    fn remove_clears_active_when_it_pointed_there() {
        let mut s = AppSettings {
            profiles: vec![profile("a"), profile("b")],
            active_profile_id: Some("a".into()),
        };
        s.remove("a");
        assert_eq!(s.profiles.len(), 1);
        assert!(s.active_profile_id.is_none());
        // Falls back to the first remaining profile.
        assert_eq!(s.active_profile().unwrap().id, "b");
    }

    #[test]
    fn active_falls_back_to_first_when_unset_or_dangling() {
        let s = AppSettings {
            profiles: vec![profile("a"), profile("b")],
            active_profile_id: Some("missing".into()),
        };
        assert_eq!(s.active_profile().unwrap().id, "a");
    }

    #[test]
    fn active_is_none_without_profiles() {
        let s = AppSettings { profiles: vec![], active_profile_id: None };
        assert!(s.active_profile().is_none());
    }
}
