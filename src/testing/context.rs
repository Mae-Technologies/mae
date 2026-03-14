//! [`TestContext`] — a generic test context for Mae-based services.
//!
//! `TestContext<C>` wraps a service-specific custom context type `C` and
//! provides access to a shared Postgres pool.  Per-test isolation is achieved
//! via transaction rollback (`pool.begin()` + drop) rather than DDL-based
//! schema creation, which avoids requiring elevated database permissions.

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

use crate::request_context::RequestContext;
use crate::session::Session;
use crate::testing::container::postgres;

/// Re-export the Postgres singleton helper.
pub use postgres::pg_singleton as pg_container;

/// Generic test context wrapping a service-specific type `C`.
#[derive(Clone)]
pub struct TestContext<C = ()> {
    /// Service-specific context, accessible from within tests.
    pub inner: C,
    /// Shared Postgres pool for this test run.
    pub pool: PgPool,
}

impl<C: Default + Clone> TestContext<C> {
    /// Build a new [`TestContext`] backed by the shared Postgres pool.
    ///
    /// Per-test isolation is achieved via transaction rollback — call
    /// `pool.begin()` at the start of each test and let the transaction
    /// drop (auto-rollback) when the test finishes.
    pub async fn new() -> Result<Self> {
        let pool = postgres::shared_pool().await?.clone();
        Ok(Self { inner: C::default(), pool })
    }
}

/// Convenience type alias matching the downstream service pattern.
pub type Ctx<C> = RequestContext<TestContext<C>>;

/// Build a [`Ctx<C>`] backed by the shared Postgres pool.
pub async fn get_context<C: Default + Clone>() -> Result<RequestContext<TestContext<C>>> {
    let base_pool = postgres::shared_pool().await?.clone();
    let pool = Arc::new(base_pool.clone());

    Ok(RequestContext::<TestContext<C>> {
        db_pool: pool,
        session: Session { user_id: 1 },
        custom: TestContext { inner: C::default(), pool: base_pool }.into()
    })
}
