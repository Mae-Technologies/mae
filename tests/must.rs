use std::panic::Location;

/// Trait for safe test assertions on `Option` and `Result`.
#[cfg(test)]
pub trait Must<T,> {
    /// Panics if the value is not as expected, with caller location.
    #[track_caller]
    fn must(self,) -> T;
}

#[cfg(test)]
impl<T,> Must<T,> for Option<T,> {
    #[track_caller]
    fn must(self,) -> T {
        self.unwrap_or_else(|| {
            panic!("test invariant failed: expected Some, got None at {}", Location::caller())
        },)
    }
}

#[cfg(test)]
impl<T, E: std::fmt::Debug,> Must<T,> for Result<T, E,> {
    #[track_caller]
    fn must(self,) -> T {
        self.unwrap_or_else(|err| {
            panic!("test invariant failed: expected Ok, got {:?} at {}", err, Location::caller())
        },)
    }
}

// ── Convenience free functions ──────────────────────────────────────────────
// Often nicer to read than method syntax in tests

#[cfg(test)]
#[track_caller]
pub fn must_be_some<T,>(opt: Option<T,>,) -> T {
    opt.must()
}

#[cfg(test)]
#[track_caller]
pub fn must_be_ok<T, E: std::fmt::Debug,>(res: Result<T, E,>,) -> T {
    res.must()
}
