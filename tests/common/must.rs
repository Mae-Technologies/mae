#[cfg(test)]
use pretty_assertions::{assert_eq, assert_ne};
use std::panic::Location;

/// Trait for safe test assertions on `Option` and `Result`.
#[cfg(test)]
pub trait Must<T,> {
    /// Panics if the value is not as expected, with caller location.
    #[track_caller]
    fn must(self,) -> T;
}

#[cfg(test)]
pub trait MustExpect<T,>: Sized {
    /// Like `expect`, but includes caller location in the panic message.
    #[track_caller]
    fn must_expect(self, msg: &str,) -> T;
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
impl<T,> MustExpect<T,> for Option<T,> {
    #[track_caller]
    fn must_expect(self, msg: &str,) -> T {
        self.unwrap_or_else(|| {
            panic!("{} (expected Some, got None) at {}", msg, Location::caller())
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

#[cfg(test)]
impl<T, E: std::fmt::Debug,> MustExpect<T,> for Result<T, E,> {
    #[track_caller]
    fn must_expect(self, msg: &str,) -> T {
        self.unwrap_or_else(|err| {
            panic!("{} (expected Ok, got {:?}) at {}", msg, err, Location::caller())
        },)
    }
}

// ── Convenience free functions ──────────────────────────────────────────────

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

#[cfg(test)]
#[track_caller]
pub fn must_expect_some<T,>(opt: Option<T,>, msg: &str,) -> T {
    opt.must_expect(msg,)
}

#[cfg(test)]
#[track_caller]
pub fn must_expect_ok<T, E: std::fmt::Debug,>(res: Result<T, E,>, msg: &str,) -> T {
    res.must_expect(msg,)
}

#[allow(clippy::disallowed_methods)]
#[track_caller]
pub fn must_eq<V: PartialEq + std::fmt::Debug,>(left: V, right: V,) {
    assert_eq!(left, right);
}

#[track_caller]
pub fn must_ne<V: PartialEq + std::fmt::Debug,>(left: V, right: V,) {
    assert_ne!(left, right);
}

#[track_caller]
pub fn must_be_true(b: bool,) {
    assert!(b);
}
