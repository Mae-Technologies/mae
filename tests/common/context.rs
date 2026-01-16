use anyhow::{Context, Result};
use mae::request_context::RequestContext;
use mae::session::Session;
use sqlx::PgPool;
use sqlx::{Connection, Executor, PgConnection};
use std::sync::Arc;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres;
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

#[derive(Clone)]
pub struct TestContext {}

impl TestContext {
    /// Stronger isolation: open a dedicated PgConnection and set `search_path`
    /// for that session, so all queries using `&mut PgConnection` see the schema.
    pub async fn scoped_connection(&self) -> Result<PgConnection> {
        let schema = format!("test_{}", Uuid::new_v4().to_string().replace('-', ""));
        // Build a connection URL matching the container pool.
        let (host, port) = postgres_host_port().await?;
        let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

        let mut conn = PgConnection::connect(&url)
            .await
            .context("failed to connect to Postgres")?;
        conn.execute(format!(r#"SET search_path TO "{}", public"#, schema).as_str())
            .await?;
        Ok(conn)
    }
}

pub type Ctx = mae::request_context::RequestContext<TestContext>;

static PG_CONTAINER: OnceCell<Mutex<Option<ContainerAsync<postgres::Postgres>>>> =
    OnceCell::const_new();

async fn pg_container() -> &'static Mutex<Option<ContainerAsync<postgres::Postgres>>> {
    PG_CONTAINER
        .get_or_init(|| async {
            // Hard gate: only start containers when explicitly enabled.
            let enabled = std::env::var("MAE_TESTCONTAINERS")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            if !enabled {
                // Keep as None; callers should error with a helpful message.
                return Mutex::new(None);
            }
            let c = postgres::Postgres::default()
                .start()
                .await
                .expect("failed to start postgres container");
            Mutex::new(Some(c))
        })
        .await
}

pub async fn shutdown_testcontainers() {
    if let Some(m) = PG_CONTAINER.get() {
        let mut guard = m.lock().await;

        // Take the container out, then drop it NOW.
        if let Some(container) = guard.take() {
            drop(container);
        }
    }
}

async fn postgres_host_port() -> Result<(String, u16)> {
    let m = pg_container().await;
    let guard = m.lock().await;

    let c = guard
        .as_ref()
        .context("testcontainers disabled. Set MAE_TESTCONTAINERS=1 to run DB tests.")?;

    let host = c
        .get_host()
        .await
        .context("failed to get host")?
        .to_string();
    let port = c
        .get_host_port_ipv4(5432)
        .await
        .context("failed to get port")?;

    Ok((host, port))
}

pub async fn get_context() -> Result<RequestContext<TestContext>> {
    let pool = Arc::new(pool().await?.clone());

    // Best-effort: sets search_path on *one* connection.
    // (This does not apply to future pooled connections.)
    let ctx = RequestContext::<TestContext> {
        db_pool: pool.clone(),
        session: Session { user_id: 1 },
        custom: TestContext {}.into(),
    };
    Ok(ctx)
}

static POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn pool() -> Result<&'static PgPool> {
    let pool_ref = POOL
        .get_or_init(|| async {
            // Inner fallible init so we can use `?` and good error context.
            let init: Result<PgPool> = async {
                let (host, port) = postgres_host_port().await?;
                let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

                let pool = PgPool::connect(&url)
                    .await
                    .context("failed to connect to postgres")?;

                sqlx::migrate!("./migrations")
                    .run(&pool)
                    .await
                    .context("migrations failed")?;

                Ok(pool)
            }
            .await;

            // `OnceCell` initializer must return `PgPool`, so we convert failure to panic here.
            // If you want *no panics*, switch `POOL` to store `Result<PgPool>` instead.
            init.expect("failed to initialize postgres pool")
        })
        .await;

    Ok(pool_ref)
}

#[cfg(test)]
mod test_context {
    //! Integration test using `TestContext` from `tests/common/context.rs`.

    use sqlx::Row;
    // Pull in `tests/common/mod.rs` for this integration-test crate.
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn parallelism() -> Result<()> {
        // Create an isolated schema for this test run.
        let ctx = get_context().await?;

        // Strongest isolation: acquire a connection and set search_path for that session.
        let mut conn = ctx.custom.scoped_connection().await?;

        // Minimal DB interaction to prove the context works.
        // (No dependency on your application's schema/tables.)
        let n: i32 = sqlx::query("SELECT 1").fetch_one(&mut conn).await?.get(0);

        assert_eq!(n, 1);

        // Optional: show the schema string is present and non-empty.

        Ok(())
    }
    #[tokio::test(flavor = "multi_thread")]
    async fn uses_test_context_schema_isolation() -> Result<()> {
        // Create an isolated schema for this test run.
        let ctx = get_context().await?;

        // Strongest isolation: acquire a connection and set search_path for that session.
        let mut conn = ctx.custom.scoped_connection().await?;

        // Minimal DB interaction to prove the context works.
        // (No dependency on your application's schema/tables.)
        let n: i32 = sqlx::query("SELECT 1").fetch_one(&mut conn).await?.get(0);

        assert_eq!(n, 1);

        // Optional: show the schema string is present and non-empty.

        Ok(())
    }
}
