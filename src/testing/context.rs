//! [`TestContext`] — a generic test context for Mae-based services.
//!
//! `TestContext<C>` wraps a service-specific custom context type `C` and
//! provides access to a shared Postgres pool plus per-test schema isolation.

use anyhow::Result;
use sqlx::{Executor, PgPool};
use std::sync::Arc;
use uuid::Uuid;

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
    /// Per-test schema name (`test_<uuid>`).
    pub schema: String
}

impl<C: Default + Clone> TestContext<C> {
    /// Build a new isolated [`TestContext`].
    pub async fn new() -> Result<Self> {
        let pool = postgres::shared_pool().await?.clone();
        let schema = spawn_scoped_schema(&pool).await?;

        Ok(Self { inner: C::default(), pool, schema })
    }

    /// Drop the scoped schema created for this test context.
    pub async fn teardown(&self) -> Result<()> {
        teardown(&self.pool, &self.schema).await
    }

    /// Open a fresh [`sqlx::PgConnection`] with `search_path` set to a unique
    /// schema, providing strong per-test schema isolation.
    pub async fn scoped_connection(&self) -> Result<sqlx::PgConnection> {
        let scope = postgres::spawn_scoped_schema().await?;
        Ok(scope.conn)
    }
}

/// Create a unique schema (`test_<uuid>`) for one test.
pub async fn spawn_scoped_schema(pool: &PgPool) -> Result<String> {
    let schema = format!("test_{}", Uuid::new_v4().simple());
    pool.execute(format!(r#"CREATE SCHEMA IF NOT EXISTS \"{schema}\""#).as_str()).await?;
    Ok(schema)
}

/// Drop a previously-created scoped schema.
pub async fn teardown(pool: &PgPool, schema: &str) -> Result<()> {
    pool.execute(format!(r#"DROP SCHEMA IF EXISTS \"{schema}\" CASCADE"#).as_str()).await?;
    Ok(())
}

/// Convenience type alias matching the downstream service pattern.
pub type Ctx<C> = RequestContext<TestContext<C>>;

/// Build a [`Ctx<C>`] backed by the shared Postgres pool.
pub async fn get_context<C: Default + Clone>() -> Result<RequestContext<TestContext<C>>> {
    let base_pool = postgres::shared_pool().await?.clone();
    let schema = spawn_scoped_schema(&base_pool).await?;

    // Keep RequestContext's db_pool as an Arc clone of the shared pool.
    let pool = Arc::new(base_pool.clone());

    Ok(RequestContext::<TestContext<C>> {
        db_pool: pool,
        session: Session { user_id: 1 },
        custom: TestContext { inner: C::default(), pool: base_pool, schema }.into()
    })
}
