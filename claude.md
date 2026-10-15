# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Current Repository State

This is a **greenfield project** — no code is scaffolded yet. The only files present are `claude.md` (this file / the construction guide) and `prd.md` (the full product requirements). Everything in the "System Construction Guide" below is the plan to build, not existing code. When starting, scaffold the Tauri v2 + Vite/React app before following the phase guide.

- `prd.md` is the authoritative product spec (features, models, UI/UX, constraints). Read it alongside this file.
- `claude.md` (below) is the architectural blueprint and phased build order. Follow the directory layout exactly.

## Commands

The project is scaffolded (Tauri v2 + Vite + React + TypeScript; Rust backend in `src-tauri/`).

**Build prerequisites (native deps):**
- `protoc` — required to build `lancedb` (`brew install protobuf`). Build fails without it.
- Rust, Node, Xcode Command Line Tools. The candle `metal` cargo feature (GPU embeddings) additionally needs the **full Xcode** toolchain to compile Metal shaders; it is OFF by default so the CPU backend builds under CLT alone.

```bash
# Frontend + full desktop app (recommended dev entry point)
npm install                 # install JS deps
npm run tauri dev           # run the full Tauri app (spawns Vite + Rust, hot-reloads both)
npm run tauri build         # produce a release desktop bundle

# Frontend only
npm run dev                 # Vite dev server (UI only, no Rust)
npm run build               # tsc typecheck + vite production build
npx tsc --noEmit            # TypeScript typecheck without emit

# Rust backend (run inside src-tauri/)
cargo check --tests         # fast type-check incl. tests
cargo test                  # run all Rust tests
cargo test <name>           # run a single test by name substring
cargo build --features metal  # build with Apple-GPU embeddings (needs full Xcode)
cargo clippy                # lint
cargo fmt                   # format
```

**Docker (reproducible deps + CI, from repo root):**
The `Dockerfile` bakes the full toolchain (Rust 1.96, Node 24, `protoc`, WebKitGTK/Tauri Linux deps) so the checks below run without a host toolchain. It does NOT build the macOS `.app`/`.dmg` or run the GUI — those need macOS + Xcode natively (`npm run tauri build`). `docker-compose.yml` keeps `node_modules`/`target` in named volumes so the Linux container never collides with the host's macOS binaries.

```bash
docker compose build                              # build the toolchain image once
docker compose run --rm dev npm install           # install JS deps in-container (first time)
docker compose run --rm dev cargo test            # run the Rust suite
docker compose run --rm dev cargo clippy --all-targets
docker compose run --rm dev npm run build         # tsc + vite build
docker compose run --rm dev                       # interactive shell
```

## Key Cross-Cutting Facts

- **External service ports (all localhost):** default local inference server on `127.0.0.1:8080` (OpenAI-compatible `/v1`); AnkiConnect on `127.0.0.1:8765`. Both are external processes the app talks to — neither is spawned by this app. Anki connection failures must degrade gracefully and never crash the Tauri thread.
- **Switchable models:** `inference/higgs_client.rs::InferenceClient` targets any OpenAI-compatible endpoint (a local server or a remote provider like OpenAI/OpenRouter/Groq/Anthropic-compat). The user manages a list of `ModelProfile`s (base URL + model + optional bearer key + temperature) persisted to `data_dir()/settings.json` (see `shared/settings.rs`; written `0600`, holds API keys in **plaintext** by design). `Router` reads the **active** profile from the shared `Arc<RwLock<AppSettings>>` on every request, so switching takes effect with no restart. Model CRUD/selection is exposed via the `list_models`/`active_model`/`set_active_model`/`save_model`/`delete_model`/`test_model` commands; the frontend UI lives in `src/features/settings/`. Embeddings stay local and are unaffected.
- **Data flow:** user input → `cognitive_core/router.rs` triages on the command prefix (`""`, `/plan`, `/deep-research`, `/socratic`) → dispatches to an agent in `cognitive_core/agents/` → pulls context from `vector_db` and/or `ingestion` → streams tokens from `inference/higgs_client.rs` back to the React `chat` feature.
- **Streaming contract:** inference streams over `tokio::sync::mpsc`; the frontend consumes via Tauri events/commands. Anki card extraction uses structured JSON-schema outputs (`front`, `back`, `tags`).
- **LanceDB storage path:** must NOT be hardcoded. `store.rs` accepts an arbitrary directory path (caller-supplied), so the DB location is configurable. Provide a sensible default in `shared/config.rs` for when none is given, but the connection API itself takes the path as a parameter.
- **IPC typing:** no `any` types are permitted across the Rust↔TS boundary; shared IPC payload structs live in `src-tauri/src/shared/models.rs` and must have matching TS types.
- **Target hardware:** M4 Apple Silicon, 24GB unified memory; embeddings (`bge-small-en-v1.5`) run on Metal/MPS. Keep idle memory under ~60MB — see the performance guardrails at the end of this file.

---

# System Construction Guide

You are building a local AI study notebook utilizing Tauri, React, Tailwind, Rust, and LanceDB, optimized for an M4 Apple Silicon environment. Follow these structural constraints and build phases strictly.

##  Feature-Based (Screaming) System Architecture Layout
Ensure the repository mirrors this feature-grouped workspace configuration exactly:

---
├── src/                                 # Frontend (React + TS + Tailwind)
│   ├── main.tsx                         # App entry point
│   ├── index.css                        # Global styles & Tailwind imports
│   ├── shared/                          # Cross-cutting UI utilities
│   │   ├── components/                  # Design system primitives (Button, Input, Modal)
│   │   ├── hooks/                       # Shared hooks (useTauriInvoke, useTheme)
│   │   └── utils/                       # Shared helpers (cn, formatters)
│   │
│   └── features/                        # Cohesive Functional Feature Domains
│       ├── workspace/                   # Left Sidebar: Document & collections list
│       │   ├── components/              # Sidebar, FileTree, UploadZone
│       │   └── hooks/useWorkspace.ts

│       ├── chat/                        # Main Conversation Engine
│       │   ├── components/              # ChatThread, MessageBubble, CommandBar
│       │   └── hooks/useChatStream.ts

│       ├── study-planner/               # Right Panel Tab: /plan viewer
│       │   └── components/              # PlanViewer, GapsList, SyllabusTree
│       ├── deep-research/               # Right Panel Tab: Research tracking
│       │   └── components/              # ResearchTerminal, LogViewer, SourceList
│       └── anki/                        # Right Panel Tab: Flashcard staging & sync
│           └── components/              # CardStagingArea, CardEditor, SyncButton
│
├── src-tauri/                           # Backend (Rust Native App Core)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs                      # App initialization & core Tauri setup
│       ├── shared/                      # Shared infrastructure cross-cutting logic
│       │   ├── config.rs                # App settings, path locators, constants
│       │   ├── error.rs                 # Centralized error mapping (AppError enum)
│       │   └── models.rs                # Core shared types and IPC payload structs
│       │
│       └── features/                    # Encapsulated Domain Subsystems
│           ├── inference/               # Local MLX (Higgs) Client & Schema Driver
│           │   ├── mod.rs               # API exposure
│           │   └── higgs_client.rs      # HTTP stream parsers
│           ├── vector_db/               # Embedded LanceDB & Local Embedding Generation
│           │   ├── mod.rs
│           │   ├── store.rs             # LanceDB connection management
│           │   └── embedder.rs          # Candle / Ort embedding pipeline
│           ├── ingestion/               # File readers & Async Crawlers
│           │   ├── mod.rs
│           │   ├── parsers/             # pdf.rs, markdown.rs, pptx.rs
│           │   └── scrapers/            # headless_web.rs, youtube_transcript.rs
│           ├── cognitive_core/          # State Machine & /commands Router
│           │   ├── mod.rs
│           │   ├── router.rs            # Core match logic for /plan, /socratic, etc.
│           │   └── agents/              # socratic.rs, planner.rs, researcher.rs
│           └── anki_sync/               # Local Agentic Anki Connect Pipeline
│               ├── mod.rs
│               └── client.rs            # HTTP client for localhost:8765
└── package.json

##  Phase-by-Phase Build Guide

### Phase 1: Local Inference Client (`features/inference`)
1. Create an OpenAI/Anthropic-compatible HTTP client inside `higgs_client.rs` that targets `http://127.0.0.1:8080/v1`.
2. Ensure it supports streaming chunks over standard async channels (`tokio::sync::mpsc`).
3. Leverage structured outputs (JSON Schema format) to guarantee strict schema adherence when calling the engine for card extraction.

### Phase 2: Embedded Storage Layer (`features/vector_db`)
1. Initialize LanceDB embedded in `store.rs`. The database directory path must be a parameter accepted by the connection API — do NOT hardcode it. Define a default path in `shared/config.rs` (used when the caller supplies none), but `store.rs` must open a LanceDB connection at any directory path passed to it.
2. Integrate a lightweight embedding pipeline using the `ort` or `candle` crates. Utilize `bge-small-en-v1.5` executing entirely on Apple's Metal Performance Shaders (MPS) framework.
3. Construct methods for inserting chunks with metadata tags (`source_file`, `timestamp`, `chunk_id`) and doing vector similarity matches returning the top 5 chunks.

### Phase 3: Processing & Scraping Engines (`features/ingestion`)
1. Build text extraction routines in `parsers/`. Implement PDF extraction (`pdf-extract`), pure Markdown file readers, and PPTX zipper-parsers.
2. Build a headless crawler in `scrapers/headless_web.rs` using `chromiumoxide` to capture dynamic text without triggering standard bot protections.
3. Implement a regex parser in `scrapers/youtube_transcript.rs` to parse YouTube IDs and extract the XML captions transcript format directly from `youtube.com/api/timedtext`.

### Phase 4: Cognitive Core & Command Router (`features/cognitive_core`)
Implement the Command Router inside `router.rs`. Intercept user messages and triage based on command strings:
*   `""` -> Match vectors -> Build RAG template -> Stream to UI.
*   `"/plan"` -> Gather document metrics -> Synthesize context structure -> Generate markdown layout.
*   `"/deep-research"` -> Initiate iterative search tasks -> Scrape pages -> Vectorize incoming content -> Print logs dynamically.
*   `"/socratic"` -> Lock down system instructions -> Restrict answers to investigative feedback questions.

### Phase 5: Automated Anki Pipeline (`features/anki_sync`)
1. Build an asynchronous client inside `client.rs` to fire structured payloads at port `8765`.
2. Wrap the API calls cleanly into `can_connect()`, `create_deck_if_missing()`, and `add_notes()`. Handle connection errors gracefully without crashing the core Tauri app thread.

### Phase 6: Frontend & Tauri Activation (`src/features/`)
1. Connect the React client to Tauri commands using `@tauri-apps/api/core`.
2. Build an efficient chat rendering interface using Tailwind CSS inside `src/features/chat/`. 
3. Include an ongoing terminal-like command line engine that auto-suggests `/plan`, `/deep-research`, and `/socratic` options when the user types a forward slash (`/`).

---

## Coding & Performance Guardrails
*   **Memory Overhead:** Keep the Rust background processes lean. Do not spawn thick persistent threads per function; use `tokio::spawn` and async network tasks where appropriate.
*   **Zero-Copy Parsers:** Ensure large document buffers are read iteratively or passed by reference (`&str` / `&[u8]`) across internal boundaries.
*   **Strict Typing:** Turn on exact TypeScript checking inside `tsconfig.json`. Do not allow the use of `any` types across IPC boundaries.