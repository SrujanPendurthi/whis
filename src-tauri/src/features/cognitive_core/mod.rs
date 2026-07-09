//! Cognitive core: the command router and its agents.
//!
//! This is the orchestration layer that ties inference, the vector index, and
//! ingestion together behind the `/command` state machine.

pub mod agents;
pub mod router;

pub use router::{parse, Command, ParsedInput, Router};
