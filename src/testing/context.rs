//! [`TestContext`] — a generic test context for Mae-based services.
//!
//! `TestContext<C>` wraps a service-specific custom context type `C` and
//! provides access to a shared Postgres pool.  Per-test isolation is achieved
//! via transaction rollback (`pool.begin()` + drop) rather than DDL-based
//! schema creation, which avoids requiring elevated database permissions.

use anyhow::Result;
use sqlx::PgPool;
use std::sync::Arc;

use crate::context::RequestContext;
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
    pub pool: PgPool
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
/// The snapshot is empty, so company grants do not apply.
pub async fn get_context<C: Default + Clone>() -> Result<RequestContext<TestContext<C>>> {
    build_context(None).await
}

/// Build a [`Ctx<C>`] with the snapshot middleware would have copied onto the request.
/// `None` is unrestricted. [`crate::grants::Grants::companies`] limits the repo key.
pub async fn get_context_with_grants<C: Default + Clone>(
    grants: Option<crate::grants::Grants>
) -> Result<RequestContext<TestContext<C>>> {
    build_context(grants).await
}

async fn build_context<C: Default + Clone>(
    grants: Option<crate::grants::Grants>
) -> Result<RequestContext<TestContext<C>>> {
    let base_pool = postgres::shared_pool().await?.clone();
    let pool = Arc::new(base_pool.clone());
    let session = Arc::new(Session(Some(1)));
    RequestContext::<TestContext<C>>::with_snapshot(
        pool,
        session,
        Arc::new(TestContext { inner: C::default(), pool: base_pool }),
        grants
    )
}
