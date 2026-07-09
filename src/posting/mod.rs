//! Multi-step posting orchestration (Command + Facade).
//!
//! Service authors compose typed [`PostingStep`]s and run them through
//! [`PostingController::attempt`]. On failure, completed steps are undone in
//! reverse order (saga compensation). Local DB locking / double-entry math
//! stay in the service; this module only orchestrates execute/undo.

mod controller;
mod step;

pub use controller::{PostingContext, PostingController, PostingReport};
pub use step::{BoxFuture, BoxPostingStep, PostingError, PostingStep, StepOutcome};
