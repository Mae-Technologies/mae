use super::step::{BoxPostingStep, PostingStep, StepOutcome};

/// Shared mutable state passed through every step.
///
/// Prefer a concrete service type for `T` (journal ids, op ids, intake refs) —
/// not a free-form JSON bag.
#[derive(Debug)]
pub struct PostingContext<T> {
    pub write_details: T,
    pub errors: Vec<String>
}

impl<T> PostingContext<T> {
    pub fn new(write_details: T) -> Self {
        Self { write_details, errors: Vec::new() }
    }

    pub fn push_error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }
}

/// Result of [`PostingController::attempt`].
#[derive(Debug, Clone)]
pub struct PostingReport {
    /// Overall success (all executes ok, or no steps).
    pub success: bool,
    /// `0` = no undo needed; `1` = undo completed; `-1` = undo failed for a step.
    pub fail_success: i8,
    /// Step names that completed successfully (before any failure).
    pub completed_steps: Vec<String>,
    /// Step that failed execute, if any.
    pub failed_step: Option<String>,
    /// Accumulated error messages (execute + undo).
    pub errors: Vec<String>
}

/// Facade that runs steps in order and undoes completed ones on failure.
pub struct PostingController<T> {
    steps: Vec<BoxPostingStep<T>>
}

impl<T> PostingController<T> {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn with_steps(steps: Vec<BoxPostingStep<T>>) -> Self {
        Self { steps }
    }

    pub fn push_step(&mut self, step: impl PostingStep<T> + 'static) {
        self.steps.push(Box::new(step));
    }

    /// Execute all steps. On first execute error, undo completed steps in reverse.
    pub async fn attempt(mut self, ctx: &mut PostingContext<T>) -> PostingReport {
        let mut completed: Vec<usize> = Vec::new();
        let mut completed_names: Vec<String> = Vec::new();

        for (idx, step) in self.steps.iter_mut().enumerate() {
            let name = step.name().to_string();
            match step.execute(ctx).await {
                Ok(StepOutcome::Completed) | Ok(StepOutcome::Skipped) => {
                    completed.push(idx);
                    completed_names.push(name);
                }
                Err(err) => {
                    ctx.push_error(format!("step `{name}` failed: {err}"));
                    let fail_success = undo_completed(&mut self.steps, &completed, ctx).await;
                    return PostingReport {
                        success: false,
                        fail_success,
                        completed_steps: completed_names,
                        failed_step: Some(name),
                        errors: ctx.errors.clone()
                    };
                }
            }
        }

        PostingReport {
            success: true,
            fail_success: 0,
            completed_steps: completed_names,
            failed_step: None,
            errors: ctx.errors.clone()
        }
    }
}

impl<T> Default for PostingController<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Undo completed steps LIFO. Returns `1` if all undos ok, `-1` if any undo fails.
async fn undo_completed<T>(
    steps: &mut [BoxPostingStep<T>],
    completed: &[usize],
    ctx: &mut PostingContext<T>
) -> i8 {
    if completed.is_empty() {
        return 0;
    }

    let mut fail_success: i8 = 1;
    for &idx in completed.iter().rev() {
        let name = steps[idx].name().to_string();
        if let Err(err) = steps[idx].undo(ctx).await {
            fail_success = -1;
            ctx.push_error(format!("undo of step `{name}` failed: {err}"));
        }
    }
    fail_success
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::posting::step::{BoxFuture, PostingError, PostingStep, StepOutcome};
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Default)]
    struct Details {
        log: Vec<String>
    }

    struct MockStep {
        name: &'static str,
        fail_execute: bool,
        fail_undo: bool,
        shared: Arc<Mutex<Vec<String>>>
    }

    impl PostingStep<Details> for MockStep {
        fn name(&self) -> &str {
            self.name
        }

        fn execute<'a>(
            &'a mut self,
            ctx: &'a mut PostingContext<Details>
        ) -> BoxFuture<'a, Result<StepOutcome, PostingError>> {
            Box::pin(async move {
                if let Ok(mut log) = self.shared.lock() {
                    log.push(format!("exec:{}", self.name));
                }
                ctx.write_details.log.push(format!("exec:{}", self.name));
                if self.fail_execute {
                    return Err(PostingError::new(format!("{} boom", self.name)));
                }
                Ok(StepOutcome::Completed)
            })
        }

        fn undo<'a>(
            &'a mut self,
            ctx: &'a mut PostingContext<Details>
        ) -> BoxFuture<'a, Result<(), PostingError>> {
            Box::pin(async move {
                if let Ok(mut log) = self.shared.lock() {
                    log.push(format!("undo:{}", self.name));
                }
                ctx.write_details.log.push(format!("undo:{}", self.name));
                if self.fail_undo {
                    return Err(PostingError::new(format!("{} undo boom", self.name)));
                }
                Ok(())
            })
        }
    }

    fn step(
        name: &'static str,
        fail_execute: bool,
        fail_undo: bool,
        shared: Arc<Mutex<Vec<String>>>
    ) -> MockStep {
        MockStep { name, fail_execute, fail_undo, shared }
    }

    #[tokio::test]
    async fn all_steps_succeed() {
        let shared = Arc::new(Mutex::new(Vec::new()));
        let mut ctl = PostingController::new();
        ctl.push_step(step("a", false, false, shared.clone()));
        ctl.push_step(step("b", false, false, shared.clone()));
        let mut ctx = PostingContext::new(Details::default());
        let report = ctl.attempt(&mut ctx).await;
        assert!(report.success);
        assert_eq!(report.fail_success, 0);
        assert_eq!(report.completed_steps, vec!["a", "b"]);
        let log = shared.lock().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(log, vec!["exec:a", "exec:b"]);
    }

    #[tokio::test]
    async fn mid_fail_undoes_in_reverse() {
        let shared = Arc::new(Mutex::new(Vec::new()));
        let mut ctl = PostingController::new();
        ctl.push_step(step("ops", false, false, shared.clone()));
        ctl.push_step(step("accounting", true, false, shared.clone()));
        let mut ctx = PostingContext::new(Details::default());
        let report = ctl.attempt(&mut ctx).await;
        assert!(!report.success);
        assert_eq!(report.fail_success, 1);
        assert_eq!(report.failed_step.as_deref(), Some("accounting"));
        assert_eq!(report.completed_steps, vec!["ops"]);
        let log = shared.lock().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(log, vec!["exec:ops", "exec:accounting", "undo:ops"]);
    }

    #[tokio::test]
    async fn undo_fail_sets_fail_success_negative() {
        let shared = Arc::new(Mutex::new(Vec::new()));
        let mut ctl = PostingController::new();
        ctl.push_step(step("ops", false, true, shared.clone()));
        ctl.push_step(step("accounting", true, false, shared.clone()));
        let mut ctx = PostingContext::new(Details::default());
        let report = ctl.attempt(&mut ctx).await;
        assert!(!report.success);
        assert_eq!(report.fail_success, -1);
        assert!(report.errors.iter().any(|e| e.contains("undo")));
    }

    #[tokio::test]
    async fn first_step_fail_no_undo() {
        let shared = Arc::new(Mutex::new(Vec::new()));
        let mut ctl = PostingController::new();
        ctl.push_step(step("ops", true, false, shared.clone()));
        let mut ctx = PostingContext::new(Details::default());
        let report = ctl.attempt(&mut ctx).await;
        assert!(!report.success);
        assert_eq!(report.fail_success, 0);
        let log = shared.lock().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(log, vec!["exec:ops"]);
    }
}
