//! [`TestContext`] — a generic test context for Mae-based services.
//!
//! `TestContext<C>` wraps a service-specific custom context type `C` and
//! provides access to the shared Postgres pool.  Services define their own `C`
//! (e.g. mock clients, feature flags) and pass it through.
//!
//! ## Usage
//! ```rust,no_run
//! use mae::testing::context::{TestContext, get_context};
//!
//! #[derive(Default, Clone)]
//! struct MyCtx { /* service-specific fields */ }
//!
//! #[tokio::test]
//! async fn my_test() {
//!     let ctx = get_context::<MyCtx>().await.unwrap();
//!     // ctx.db_pool — shared pool
//!     // ctx.custom.inner — your MyCtx
//! }
//! ```

use anyhow::Result;
use std::sync::Arc;

use crate::request_context::RequestContext;
use crate::session::Session;
use crate::testing::container::postgres;

// Re-export the free functions that lived here pre-refactor so existing
// consumers don't need to update their imports.
pub use postgres::{pg_singleton as pg_container, spawn_scoped_schema, teardown};

// ── TestContext ───────────────────────────────────────────────────────────────

/// Generic test context wrapping a service-specific type `C`.
///
/// `C` is the same type used as the type parameter of [`RequestContext`] in the
/// service under test.  In tests you typically define a lightweight `struct
/// TestCustom { … }` that implements [`Default`] and [`Clone`].
#[derive(Clone,)]
pub struct TestContext<C,> {
    /// Service-specific context, accessible from within tests.
    pub inner: C,
}

impl<C: Default + Clone,> TestContext<C,> {
    /// Open a fresh [`sqlx::PgConnection`] with `search_path` set to a unique
    /// schema, providing strong per-test schema isolation.
    pub async fn scoped_connection(&self,) -> Result<sqlx::PgConnection,> {
        let scope = spawn_scoped_schema().await?;
        Ok(scope.conn,)
    }
}

/// Convenience type alias matching the downstream service pattern.
pub type Ctx<C,> = RequestContext<TestContext<C,>,>;

// ── Context builder ───────────────────────────────────────────────────────────

/// Build a [`Ctx<C>`] backed by the shared Postgres pool.
///
/// `C` must implement [`Default`] (used to construct [`TestContext::inner`]).
pub async fn get_context<C: Default + Clone,>() -> Result<RequestContext<TestContext<C,>,>,> {
    let pool = postgres::shared_pool().await?;
    let pool = Arc::new(pool.clone(),);

    Ok(RequestContext::<TestContext<C,>,> {
        db_pool: pool,
        session: Session { user_id: 1, },
        custom: TestContext { inner: C::default(), }.into(),
    },)
}
