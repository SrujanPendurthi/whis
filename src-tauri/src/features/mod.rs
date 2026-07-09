//! Encapsulated domain subsystems. Each feature owns its data flow and exposes
//! a small API through its `mod.rs`.

pub mod anki_sync;
pub mod cognitive_core;
pub mod inference;
pub mod ingestion;
pub mod vector_db;
