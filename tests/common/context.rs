use super::{env, must::MustExpect};
use anyhow::{Context, Result};
use mae::request_context::RequestContext;
use mae::session::Session;
use sqlx::PgPool;
use sqlx::{Connection, Executor, PgConnection};
use std::io::IsTerminal;
use std::sync::Arc;
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, ImageExt};
use testcontainers_modules::postgres::Postgres;
use tokio::process::Command;
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
        let pool_opt = pg_container().await.lock().await;
        let pool = pool_opt.as_ref().ok_or_else(|| anyhow::anyhow!("Unable to init pool"))?;

        let url = &env::load().app_database_url_with_port(pool.port);
        let mut conn = PgConnection::connect(url).await.context("failed to connect to Postgres")?;
        conn.execute(format!(r#"SET search_path TO "{}""#, schema).as_str()).await?;
        Ok(conn)
    }
}

pub type Ctx = mae::request_context::RequestContext<TestContext>;

pub struct TestContainer {
    pub container: ContainerAsync<Postgres>,
    pub id: String,
    pub port: u16
}

static PG_CONTAINER: OnceCell<Mutex<Option<TestContainer>>> = OnceCell::const_new();

async fn pg_container() -> &'static Mutex<Option<TestContainer>> {
    PG_CONTAINER
        .get_or_init(|| async {
            let id = format!("test_container_{}", Uuid::new_v4().to_string().replace('-', ""));
            // Hard gate: only start containers when explicitly enabled.
            let enabled = std::env::var("MAE_TESTCONTAINERS")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            let _conf = env::load();

            if !enabled {
                // Keep as None; callers should error with a helpful message.
                return Mutex::new(None);
            }
            let conf = env::load();

            let container: ContainerAsync<Postgres> = Postgres::default()
                .with_user(conf.superuser.as_str())
                .with_password(conf.superuser_pwd.as_str())
                .with_db_name(conf.app_db_name.as_str())
                .with_container_name(&id)
                .start()
                .await
                .must_expect("failed to start postgres container");

            // WARN: hard-coded port here -- this is the fefault port to pg
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .must_expect("failed to get postgres mapped port");
            Mutex::new(Some(TestContainer { container, id, port }))
        })
        .await
}

pub async fn teardown() {
    if let Some(m) = PG_CONTAINER.get() {
        let mut guard = m.lock().await;

        // Take the container out, then drop it NOW.
        if let Some(container) = guard.take() {
            drop(container);
        }
    }
}

async fn _postgres_host_port() -> Result<(String, u16)> {
    let m = pg_container().await;
    let guard = m.lock().await;

    let c = guard
        .as_ref()
        .context("testcontainers disabled. Set MAE_TESTCONTAINERS=1 to run DB tests.")?;

    let host = c.container.get_host().await.context("failed to get host")?.to_string();
    let port = c.container.get_host_port_ipv4(5432).await.context("failed to get port")?;

    Ok((host, port))
}

pub async fn get_context() -> Result<RequestContext<TestContext>> {
    let pool = Arc::new(pool().await?.clone());

    // Best-effort: sets search_path on *one* connection.
    // (This does not apply to future pooled connections.)
    let ctx = RequestContext::<TestContext> {
        db_pool: pool.clone(),
        session: Session { user_id: 1 },
        custom: TestContext {}.into()
    };
    Ok(ctx)
}

static POOL: OnceCell<PgPool> = OnceCell::const_new();

async fn pool() -> Result<&'static PgPool> {
    let pool_ref = POOL
        .get_or_init(|| async {
            // Inner fallible init so we can use `?` and good error context.
            let init: Result<PgPool> = async {
                let m = pg_container().await;
                let guard = m.lock().await;

                let c = guard.as_ref().context(
                    "testcontainers disabled. Set MAE_TESTCONTAINERS=1 to run DB tests."
                )?;

                // RUN Presql scripts
                run_premigration(c).await.context("pre-migrations script failed")?;

                let _cfg = env::load();

                // Get a pool with app_user priviledges to pass to the caller
                let app_url = &env::load().app_database_url_with_port(c.port);
                let pool =
                    PgPool::connect(app_url).await.context("failed to connect to postgres")?;

                Ok(pool)
            }
            .await;

            // `OnceCell` initializer must return `PgPool`, so we convert failure to panic here.
            // If you want *no panics*, switch `POOL` to store `Result<PgPool>` instead.
            init.must_expect("failed to initialize postgres pool")
        })
        .await;

    Ok(pool_ref)
}

async fn run_premigration(container: &TestContainer) -> Result<()> {
    let script_path = "./scripts/sqlx_premigration.sh";

    let stdout_is_tty = std::io::stdout().is_terminal();
    let stderr_is_tty = std::io::stderr().is_terminal();

    let mut cmd = Command::new("bash");
    cmd.arg(script_path);

    let port = container.container.get_host_port_ipv4(5432).await.context("failed to get port")?;
    let env = env::load();

    // TODO: this is too much, just make the script get a DB_PORT OVERRIDE
    cmd.env("NO_DOT_ENV", "1");
    cmd.env("CONTAINER", &container.id);
    cmd.env("DEBUG", "1");
    cmd.env("RUN_APP_MIGRATIONS", "1");
    cmd.env("TTY_OVERRIDE", "1");
    cmd.env("ADMIN_MIGRATIONS_PATH", &env.admin_migrations_path);
    cmd.env("APP_MIGRATIONS_PATH", &env.app_migrations_path);
    cmd.env("DB_HOST", &env.db_host);
    cmd.env("DB_PORT", port.to_string());
    cmd.env("APP_DB_NAME", &env.app_db_name);
    cmd.env("SUPER_USER", &env.superuser);
    cmd.env("SUPERUSER_PWD", &env.superuser_pwd);
    cmd.env("MIGRATOR_USER", &env.migrator_user);
    cmd.env("MIGRATOR_PWD", &env.migrator_pwd);
    cmd.env("APP_USER", &env.app_user);
    cmd.env("APP_USER_PWD", &env.app_user_pwd);
    cmd.env("TABLE_PROVISIONER_USER", &env.table_provisioner_user);
    cmd.env("TABLE_PROVISIONER_PWD", &env.table_provisioner_pwd);
    cmd.env("SUPER_DATABASE_URL", env.super_database_url_with_port(port));
    cmd.env("SEARCH_PATH", &env.search_path);
    cmd.env("DATABASE_URL", env.database_url_with_port(port));
    cmd.env("APP_DATABASE_URL", env.app_database_url_with_port(port));
    cmd.env("TABLE_CREATOR_DATABASE_URL", env.table_creator_database_url_with_port(port));

    // cmd.stderr(std::process::Stdio::inherit(),);
    // cmd.stdout(std::process::Stdio::inherit(),);
    if stdout_is_tty {
        cmd.stdout(std::process::Stdio::inherit());
    } else {
        cmd.stdout(std::process::Stdio::piped());
    }

    if stderr_is_tty {
        cmd.stderr(std::process::Stdio::inherit());
    } else {
        cmd.stderr(std::process::Stdio::piped());
    }

    // let output =
    //     cmd.status().await.with_context(|| format!("failed to spawn script: {script_path}"),)?;
    //
    // if !output.success() {
    //     anyhow::bail!("script exited with status: {:?}", output.code());
    // }

    let output =
        cmd.output().await.with_context(|| format!("failed to spawn script: {script_path}"))?;

    if !output.status.success() {
        // Only dump captured output when non-TTY; in TTY mode it was already streamed.
        if !stdout_is_tty && !output.stdout.is_empty() {
            eprintln!("--- script stdout ---\n{}", String::from_utf8_lossy(&output.stdout));
        }
        if !stderr_is_tty && !output.stderr.is_empty() {
            eprintln!("--- script stderr ---\n{}", String::from_utf8_lossy(&output.stderr));
        }

        anyhow::bail!("script exited with status: {}", output.status);
    }

    Ok(())
}

#[cfg(test)]
mod test_context {
    //! Integration test using `TestContext` from `tests/common/context.rs`.

    use crate::common::must::must_eq;
    use sqlx::Row;

    // Pull in `tests/common/mod.rs` for this integration-test crate.
    use super::get_context;
    use anyhow::Result;
    use mae_macros::mae_test;

    #[cfg_attr(miri, ignore)]
    #[mae_test]
    async fn parallelism() -> Result<()> {
        // Create an isolated schema for this test run.
        let ctx = get_context().await?;

        // Strongest isolation: acquire a connection and set search_path for that session.
        let mut conn = ctx.custom.scoped_connection().await?;

        // Minimal DB interaction to prove the context works.
        // (No dependency on your application's schema/tables.)
        let n: i32 = sqlx::query("SELECT 1").fetch_one(&mut conn).await?.get(0);

        must_eq(n, 1);

        // Optional: show the schema string is present and non-empty.

        Ok(())
    }

    #[cfg_attr(miri, ignore)]
    #[mae_test]
    async fn uses_test_context_schema_isolation() -> Result<()> {
        // Create an isolated schema for this test run.
        let ctx = get_context().await?;

        // Strongest isolation: acquire a connection and set search_path for that session.
        let mut conn = ctx.custom.scoped_connection().await?;

        // Minimal DB interaction to prove the context works.
        // (No dependency on your application's schema/tables.)
        let n: i32 = sqlx::query("SELECT 1").fetch_one(&mut conn).await?.get(0);

        must_eq(n, 1);

        // Optional: show the schema string is present and non-empty.

        Ok(())
    }
    // TODO: testing the db should be done (DDL, DML, limits) -> dont to this in rust, create a
    // script to run tests with the pgtap extension.
}
