//! whis backend library entry point.
//!
//! Wires the shared infrastructure and feature subsystems into the Tauri app
//! and registers the IPC command handlers. The cognitive-core [`Router`] (which
//! owns the inference client + vector index) is initialized lazily on first use
//! so app launch stays fast and doesn't require the local model until needed.

pub mod features;
pub mod shared;

use tauri::{Emitter, Manager};
use tokio::sync::{mpsc, OnceCell, RwLock};

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::features::anki_sync::AnkiClient;
use crate::features::cognitive_core::Router;
use crate::features::inference::{GenerationParams, InferenceClient};
use crate::features::vector_db::{VectorIndex, VectorStore};
use crate::shared::error::{AppError, AppResult};
use crate::shared::models::{AppSettings, Card, ChatMessage, ModelProfile, SourceInfo, StreamEvent};
use crate::shared::{config, settings};

/// Channel depth for backend→frontend event forwarding.
const EVENT_CHANNEL_CAPACITY: usize = 64;
/// Upper bound on chunks scanned when listing workspace sources.
const SOURCE_SCAN_LIMIT: usize = 100_000;

/// Managed application state. Model-selection [`settings`] are loaded eagerly at
/// startup and shared with the [`Router`]; the `Router` (which loads the
/// embedding model) and a model-free read-only [`VectorStore`] are each built on
/// first access.
#[derive(Default)]
struct AppState {
    settings: Arc<RwLock<AppSettings>>,
    router: OnceCell<Router>,
    store: OnceCell<VectorStore>,
}

/// Lazily build (once) and borrow the shared [`Router`]. First call loads the
/// embedding model and opens the database at the default path. The active
/// inference model is resolved per-request from the shared settings, so this is
/// built only once regardless of later model switches.
async fn ensure_router(state: &AppState) -> AppResult<&Router> {
    state
        .router
        .get_or_try_init(|| async {
            let index = VectorIndex::open(config::default_db_path()?).await?;
            Ok::<Router, AppError>(Router::new(index, state.settings.clone()))
        })
        .await
}

/// Build a client + generation params from the active model profile.
async fn active_client(state: &AppState) -> AppResult<(InferenceClient, GenerationParams)> {
    let guard = state.settings.read().await;
    let profile = guard
        .active_profile()
        .ok_or_else(|| AppError::Config("no model profile configured".into()))?;
    Ok((InferenceClient::from_profile(profile), GenerationParams::from_profile(profile)))
}

/// Lazily open (once) a read-only store connection that does NOT load the
/// embedding model — used for cheap metadata queries like listing sources.
async fn ensure_store(state: &AppState) -> AppResult<&VectorStore> {
    state
        .store
        .get_or_try_init(|| async { VectorStore::connect(config::default_db_path()?).await })
        .await
}

/// Spawn a task that forwards stream events to the given window channel until
/// the sender is dropped. Returns the sender half and the task handle.
fn spawn_event_forwarder(
    window: tauri::Window,
    channel: String,
) -> (mpsc::Sender<StreamEvent>, tokio::task::JoinHandle<()>) {
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(EVENT_CHANNEL_CAPACITY);
    let handle = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if window.emit(&channel, &event).is_err() {
                break; // window gone
            }
        }
    });
    (tx, handle)
}

/// Local endpoints the frontend can display / health-check. `inference` and
/// `activeModel` reflect the currently selected model profile.
#[tauri::command]
async fn service_endpoints(state: tauri::State<'_, AppState>) -> AppResult<serde_json::Value> {
    let guard = state.settings.read().await;
    let active = guard.active_profile();
    Ok(serde_json::json!({
        "inference": active.map(|p| p.base_url.clone()).unwrap_or_default(),
        "activeModel": active.map(|p| p.label.clone()).unwrap_or_default(),
        "anki": config::ANKI_CONNECT_URL,
        "embeddingModel": config::EMBEDDING_MODEL_ID,
    }))
}

/// List all configured model profiles.
#[tauri::command]
async fn list_models(state: tauri::State<'_, AppState>) -> AppResult<Vec<ModelProfile>> {
    Ok(state.settings.read().await.profiles.clone())
}

/// Id of the currently active model profile (falls back to the first).
#[tauri::command]
async fn active_model(state: tauri::State<'_, AppState>) -> AppResult<Option<String>> {
    Ok(state.settings.read().await.active_profile().map(|p| p.id.clone()))
}

/// Select the active model profile and persist the choice.
#[tauri::command]
async fn set_active_model(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let mut guard = state.settings.write().await;
    guard.active_profile_id = Some(id);
    settings::save(&guard)
}

/// Insert or update a model profile and persist.
#[tauri::command]
async fn save_model(state: tauri::State<'_, AppState>, profile: ModelProfile) -> AppResult<()> {
    let mut guard = state.settings.write().await;
    guard.upsert(profile);
    settings::save(&guard)
}

/// Delete a model profile and persist (clears the active pointer if it matched).
#[tauri::command]
async fn delete_model(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let mut guard = state.settings.write().await;
    guard.remove(&id);
    settings::save(&guard)
}

/// Validate a profile's endpoint + auth with a lightweight `GET /models` probe.
#[tauri::command]
async fn test_model(state: tauri::State<'_, AppState>, id: String) -> AppResult<()> {
    let client = {
        let guard = state.settings.read().await;
        let profile = guard
            .profiles
            .iter()
            .find(|p| p.id == id)
            .ok_or_else(|| AppError::Config(format!("unknown model profile: {id}")))?;
        InferenceClient::from_profile(profile)
    };
    client.probe().await
}

/// Route a user input through the cognitive core, streaming `StreamEvent`s on
/// the `channel` event name. Handles `""`, `/plan`, `/deep-research`, and
/// `/socratic`. Emits a terminal `Done`/`Error` event when finished.
#[tauri::command]
async fn run_command(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
    channel: String,
    input: String,
) -> AppResult<()> {
    let router = ensure_router(&state).await?;
    let (tx, forwarder) = spawn_event_forwarder(window, channel);

    let result = router.handle(&input, &tx).await;
    let terminal = match &result {
        Ok(()) => StreamEvent::Done,
        Err(e) => StreamEvent::Error { message: e.to_string() },
    };
    let _ = tx.send(terminal).await;
    drop(tx);
    let _ = forwarder.await;
    result
}

/// Ingest a local file (md/pdf/pptx/txt) into the shared index. Returns the
/// number of chunks stored.
#[tauri::command]
async fn ingest_file(state: tauri::State<'_, AppState>, path: String) -> AppResult<usize> {
    ensure_router(&state).await?.ingest_file(&path).await
}

/// Scrape and ingest a web page. Returns the number of chunks stored.
#[tauri::command]
async fn ingest_web(state: tauri::State<'_, AppState>, url: String) -> AppResult<usize> {
    ensure_router(&state).await?.ingest_web(&url).await
}

/// Fetch and ingest a YouTube transcript. Returns the number of chunks stored.
#[tauri::command]
async fn ingest_youtube(state: tauri::State<'_, AppState>, url: String) -> AppResult<usize> {
    ensure_router(&state).await?.ingest_youtube(&url).await
}

/// List distinct ingested sources with their chunk counts. Model-free, so the
/// workspace sidebar works even when the inference model isn't available.
#[tauri::command]
async fn list_sources(state: tauri::State<'_, AppState>) -> AppResult<Vec<SourceInfo>> {
    let chunks = ensure_store(&state).await?.scan(SOURCE_SCAN_LIMIT).await?;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for c in &chunks {
        *counts.entry(c.metadata.source_file.clone()).or_insert(0) += 1;
    }
    Ok(counts
        .into_iter()
        .map(|(source_file, chunk_count)| SourceInfo { source_file, chunk_count })
        .collect())
}

/// Generate retrieval-grounded Anki cards for a topic. The cards are returned
/// for the user to review in the staging area before syncing.
#[tauri::command]
async fn generate_cards(state: tauri::State<'_, AppState>, topic: String) -> AppResult<Vec<Card>> {
    ensure_router(&state).await?.generate_cards(&topic).await
}

/// Whether the local Anki desktop app (with AnkiConnect) is reachable.
#[tauri::command]
async fn anki_status() -> bool {
    AnkiClient::new().can_connect().await
}

/// Push reviewed cards into `deck`, creating the deck if needed. Returns the
/// per-card note ids (`null` where Anki rejected a note, e.g. a duplicate).
#[tauri::command]
async fn anki_sync_cards(deck: String, cards: Vec<Card>) -> AppResult<Vec<Option<i64>>> {
    AnkiClient::new().sync_cards(&deck, &cards).await
}

/// Direct, un-routed streaming chat against the local engine (no retrieval).
/// Kept for simple chat and diagnostics; `run_command` is the primary entry.
#[tauri::command]
async fn chat_stream(
    window: tauri::Window,
    state: tauri::State<'_, AppState>,
    channel: String,
    messages: Vec<ChatMessage>,
) -> AppResult<()> {
    let (client, params) = active_client(&state).await?;
    let mut rx = client.stream_chat(messages, params).await?;
    let (tx, forwarder) = spawn_event_forwarder(window, channel);

    tokio::spawn(async move {
        while let Some(item) = rx.recv().await {
            let event = match item {
                Ok(text) => StreamEvent::Token { text },
                Err(e) => StreamEvent::Error { message: e.to_string() },
            };
            if tx.send(event).await.is_err() {
                return;
            }
        }
        let _ = tx.send(StreamEvent::Done).await;
    });

    let _ = forwarder.await;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Load persisted model settings; fall back to defaults on any error
            // so a malformed file never blocks startup.
            let loaded = settings::load().unwrap_or_default();
            app.manage(AppState {
                settings: Arc::new(RwLock::new(loaded)),
                ..Default::default()
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            service_endpoints,
            run_command,
            ingest_file,
            ingest_web,
            ingest_youtube,
            list_sources,
            generate_cards,
            anki_status,
            anki_sync_cards,
            chat_stream,
            list_models,
            active_model,
            set_active_model,
            save_model,
            delete_model,
            test_model
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
