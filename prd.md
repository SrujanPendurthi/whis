# Product Requirements Document (PRD): Local Study AI Engine

## 1. Overview & Objective
A privacy-first, ultra-fast, local alternative to NotebookLM optimized for Apple Silicon (M4, 24GB Unified Memory). The system ingests local documents and external links to act as an agentic study companion capable of custom command states (`/plan`, `/deep-research`), active Socratic interrogation, and zero-friction flashcard generation synced directly to Anki.

## 2. Tech Stack & Constraints
*   **Frontend:** Tauri v2 + React + TypeScript + Tailwind CSS.
*   **Backend:** Rust (Native Tauri Core).
*   **Vector Database:** LanceDB (Embedded in Rust backend).
*   **Inference Engine:** Higgs (Single-binary Rust native MLX inference server exposing an OpenAI/Anthropic-compatible API on `127.0.0.1:8080`).
*   **Target Models:** 
    *   *Primary Reasoning:* Llama 3.1 8B Instruct or Qwen 2.5 7B Instruct (4-bit/8-bit MLX).
    *   *Auxiliary/Routing:* Qwen 2.5 3B Instruct (4-bit MLX).
*   **Local Dependencies:** Anki desktop client running the `AnkiConnect` plugin active on port 8765.

---

## 3. Core Feature Requirements

### 3.1 Data Ingestion & Processing Pipeline
*   **Markdown:** Native parsing, splitting chunks cleanly by headers (`#`, `##`).
*   **PDFs:** Extract text content natively via the `pdf-extract` or `lopdf` Rust crate.
*   **PowerPoints (.pptx):** Text extraction from slides via local structural zip extraction of the underlying XML.
*   **YouTube Links:** Extract video IDs via regex; fetch raw transcripts asynchronously via local scrapers without hitting official Google APIs.
*   **Web Scraping:** Headless browser integration via `chromiumoxide` or `headless_chrome` for rendering heavily dynamic Javascript pages.

### 3.2 Cognitive Core & State Router (Command Modes)
The system intercepts the input string in the Rust backend to check for command flags:
*   **Standard RAG (No Flag):** Queries LanceDB, pulls top $k$ relevant context blocks, feeds context to the primary model, and streams the answer to the UI.
*   **`/plan`:** Switches system to context synthesis mode. Disables conversational chat logs. Aggregates metadata across the entire workspace to output a structured markdown study syllabus.
*   **`/deep-research`:** Triggers a multi-turn autonomous loop using the routing engine. Breaks a topic down into multiple search queries, scrapes web targets via the headless browser, writes findings to LanceDB, and generates a unified deep-dive report.
*   **`/socratic`:** Mutates system prompts to force the model into an interrogative persona. Instead of providing answers, it cross-references user input against the LanceDB semantic index to find flaws, logical gaps, or missing evidence, asking exactly one challenging question back.

### 3.3 Agentic Anki Integration
*   The system must parse model outputs using strict JSON formatting (`front`, `back`, `tags`).
*   The Rust backend communicates directly with the active local Anki app over `http://127.0.0.1:8765`.
*   Automatically checks if the target deck exists; if not, creates it natively before pushing cards.

---

## 4. UI/UX Specifications
*   **Desktop Window:** Single-window lightweight application footprint (< 60MB idle memory).
*   **Sidebar Layout:** Workspace navigator displaying ingested files, active collections, and toggles for active knowledge bases.
*   **Main Workspace Panels:**
    *   *Left Panel:* Interactive chat thread supporting terminal-like commands (`/plan`, `/socratic`, `/deep-research`).
    *   *Right Panel:* Tabbed auxiliary view containing current study blueprints, active research crawl logs, or a staging area for newly generated Anki cards before injection.