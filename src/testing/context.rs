//! [`TestContext`] — a generic test context for Mae-based services.
//!
//! `TestContext<C>` wraps a service-specific custom context type `C` and
//! provides access to a shared Postgres pool.  Per-test isolation is achieved
//! via transaction rollback (`pool.begin()` + drop) rather than DDL-based
//! schema creation, which avoids requiring elevated database permissions.

use anyhow::Result;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::PgPool;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::context::RequestContext;
use crate::grants::{GrantConfig, Grants};
use crate::session::Session;
use crate::testing::container::postgres;

/// Re-export the Postgres singleton helper.
pub use postgres::pg_singleton as pg_container;

/// Generic test context wrapping a service-specific type `C`.
///
/// `S` is [`GrantConfig::Snapshot`](crate::grants::GrantConfig::Snapshot).
/// It defaults to `()`, an empty snapshot.
#[derive(Clone)]
pub struct TestContext<C = (), S = ()> {
    /// Service-specific context, accessible from within tests.
    pub inner: C,
    /// Shared Postgres pool for this test run.
    pub pool: PgPool,
    _snapshot: PhantomData<S>
}

impl<C, S> GrantConfig for TestContext<C, S>
where
    C: Clone + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static + Serialize + DeserializeOwned
{
    type Snapshot = S;
}

impl<C: Default + Clone> TestContext<C> {
    /// Build a new [`TestContext`] backed by the shared Postgres pool.
    ///
    /// Per-test isolation is achieved via transaction rollback — call
    /// `pool.begin()` at the start of each test and let the transaction
    /// drop (auto-rollback) when the test finishes.
    pub async fn new() -> Result<Self> {
        let pool = postgres::shared_pool().await?.clone();
        Ok(Self { inner: C::default(), pool, _snapshot: PhantomData })
    }
}

/// Convenience type alias matching the downstream service pattern.
pub type Ctx<C, S = ()> = RequestContext<TestContext<C, S>>;

/// Build a [`Ctx<C>`] backed by the shared Postgres pool.
/// The snapshot is open, so key filters add no predicate.
pub async fn get_context<C, S>() -> Result<RequestContext<TestContext<C, S>>>
where
    C: Default + Clone + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static + Serialize + DeserializeOwned
{
    build_context(None).await
}

/// Build a [`Ctx`] with the snapshot middleware would have put on the session.
/// `None` is unrestricted. `Some` is the application document `S`.
pub async fn get_context_with_grants<C, S>(
    grants: Option<Grants<S>>
) -> Result<RequestContext<TestContext<C, S>>>
where
    C: Default + Clone + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static + Serialize + DeserializeOwned
{
    build_context(grants).await
}

async fn build_context<C, S>(grants: Option<Grants<S>>) -> Result<RequestContext<TestContext<C, S>>>
where
    C: Default + Clone + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static + Serialize + DeserializeOwned
{
    let base_pool = postgres::shared_pool().await?.clone();
    let pool = Arc::new(base_pool.clone());
    let grants = grants.unwrap_or_else(Grants::open);
    let session = Arc::new(Session { user: Some(1), roles: grants });
    RequestContext::<TestContext<C, S>>::new(
        pool,
        session,
        Arc::new(TestContext { inner: C::default(), pool: base_pool, _snapshot: PhantomData })
    )
    .await
}
