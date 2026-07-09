//! Local agentic Anki pipeline: pushes generated flashcards to the running Anki
//! desktop app via AnkiConnect.

pub mod client;

pub use client::AnkiClient;
