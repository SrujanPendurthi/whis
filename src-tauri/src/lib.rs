//! whis backend library entry point.
//!
//! Wires the shared infrastructure and feature subsystems into the Tauri app
//! and registers the IPC command handlers. The cognitive-core [`Router`] (which
//! owns the inference client + vector index) is initialized lazily on first use
//! so app launch stays fast and doesn't require the local model until needed.

pub mod features;
pub mod shared;

use tauri::{Emitter, Manager};
use tokio::sync::{mpsc, OnceCell};

use std::collections::BTreeMap;

use crate::features::anki_sync::AnkiClient;
use crate::features::cognitive_core::Router;
use crate::features::inference::{GenerationParams, HiggsClient};
use crate::features::vector_db::{VectorIndex, VectorStore};
use crate::shared::config;
use crate::shared::error::{AppError, AppResult};
use crate::shared::models::{Card, ChatMessage, SourceInfo, StreamEvent};

/// Channel depth for backend→frontend event forwarding.
const EVENT_CHANNEL_CAPACITY: usize = 64;
/// Upper bound on chunks scanned when listing workspace sources.
const SOURCE_SCAN_LIMIT: usize = 100_000;

/// Managed application state. The [`Router`] (which loads the embedding model)
/// and a model-free read-only [`VectorStore`] are each built on first access.
#[derive(Default)]
struct AppState {
    router: OnceCell<Router>,
    store: OnceCell<VectorStore>,
}

/// Lazily build (once) and borrow the shared [`Router`]. First call loads the
/// embedding model and opens the database at the default path.
async fn ensure_router(state: &AppState) -> AppResult<&Router> {
    state
        .router
        .get_or_try_init(|| async {
            let client = HiggsClient::new();
            let index = VectorIndex::open(config::default_db_path()?).await?;
            Ok::<Router, AppError>(Router::new(client, index))
        })
        .await
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

/// Local endpoints the frontend can display / health-check.
#[tauri::command]
fn service_endpoints() -> serde_json::Value {
    serde_json::json!({
        "inference": config::HIGGS_BASE_URL,
        "anki": config::ANKI_CONNECT_URL,
        "embeddingModel": config::EMBEDDING_MODEL_ID,
    })
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
    channel: String,
    messages: Vec<ChatMessage>,
) -> AppResult<()> {
    let client = HiggsClient::new();
    let mut rx = client.stream_chat(messages, GenerationParams::default()).await?;
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
        .setup(|app| {
            app.manage(AppState::default());
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
            chat_stream
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
