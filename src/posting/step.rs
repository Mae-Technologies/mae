use std::future::Future;
use std::pin::Pin;

use super::controller::PostingContext;

/// Boxed async step future (avoids requiring `async_trait` on the framework).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Outcome of a successful [`PostingStep::execute`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepOutcome {
    /// Step performed work and may need undo on later failure.
    Completed,
    /// Step was a no-op (e.g. idempotent hit); still recorded if needed.
    Skipped
}

/// Error from a posting step. Controllers collect messages into the report.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct PostingError {
    pub message: String
}

impl PostingError {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl From<anyhow::Error> for PostingError {
    fn from(err: anyhow::Error) -> Self {
        Self { message: err.to_string() }
    }
}

impl From<String> for PostingError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for PostingError {
    fn from(message: &str) -> Self {
        Self { message: message.to_string() }
    }
}

/// One atomic unit of a multi-step post (Command pattern).
///
/// Implement with typed write-details `T` held in [`PostingContext`].
pub trait PostingStep<T>: Send {
    fn name(&self) -> &str;

    fn execute<'a>(
        &'a mut self,
        ctx: &'a mut PostingContext<T>
    ) -> BoxFuture<'a, Result<StepOutcome, PostingError>>;

    fn undo<'a>(
        &'a mut self,
        ctx: &'a mut PostingContext<T>
    ) -> BoxFuture<'a, Result<(), PostingError>>;
}

/// Type-erased step for building a controller from heterogeneous adapters.
pub type BoxPostingStep<T> = Box<dyn PostingStep<T>>;
