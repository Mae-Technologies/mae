//! Postgres container singleton + per-test schema isolation.

use crate::testing::{env, must::MustExpect};
use anyhow::{Context, Result};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::io::IsTerminal;
use std::sync::Arc;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::process::Command;
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

use super::MaeContainer;

// ── Singleton ─────────────────────────────────────────────────────────────────

pub struct Inner {
    pub container: ContainerAsync<GenericImage>,
    pub id: String,
    pub port: u16
}

static SINGLETON: OnceCell<Mutex<Option<Inner>>> = OnceCell::const_new();
static POOL: OnceCell<PgPool> = OnceCell::const_new();

// ── Public isolation scope ────────────────────────────────────────────────────

/// Per-test isolation scope for Postgres.
///
/// Owns a [`PgConnection`] with `search_path` set to a unique schema,
/// preventing cross-test interference even when tests run in parallel.
pub struct PgScope {
    /// Schema name (e.g. `test_<uuid>`).
    pub schema: String,
    /// Connection whose `search_path` is scoped to [`schema`](Self::schema).
    pub conn: PgConnection
}

// ── MaeContainer impl ─────────────────────────────────────────────────────────

/// Zero-sized handle for the Postgres container singleton.
pub struct PostgresContainer;

impl MaeContainer for PostgresContainer {
    type Scope = PgScope;

    async fn start() -> Option<()> {
        pg_singleton().await.lock().await.as_ref().map(|_| ())
    }

    async fn scope() -> Result<PgScope> {
        spawn_scoped_schema().await
    }

    async fn teardown() {
        teardown().await;
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Returns (or lazily initialises) the shared Postgres container.
///
/// Guarded by `MAE_TESTCONTAINERS=1`.  The inner `Option` is `None` when the
/// guard is absent.
pub async fn pg_singleton() -> &'static Mutex<Option<Inner>> {
    SINGLETON
        .get_or_init(|| async {
            let enabled = std::env::var("MAE_TESTCONTAINERS")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            // Always load env (validates required vars are set).
            let _conf = env::load();

            if !enabled {
                return Mutex::new(None);
            }

            let conf = env::load();
            let id = format!("mae_pg_{}", Uuid::new_v4().to_string().replace('-', ""));

            let image = GenericImage::new("ghcr.io/mae-technologies/postgres-mae", "latest")
                .with_exposed_port(5432.tcp())
                .with_wait_for(WaitFor::message_on_stderr(
                    "database system is ready to accept connections"
                ));

            let container: ContainerAsync<GenericImage> = image
                .with_env_var("POSTGRES_DB", conf.app_db_name.as_str())
                .with_env_var("POSTGRES_USER", conf.superuser.as_str())
                .with_env_var("POSTGRES_PASSWORD", conf.superuser_pwd.as_str())
                .with_env_var("MIGRATOR_USER", conf.migrator_user.as_str())
                .with_env_var("MIGRATOR_PASSWORD", conf.migrator_pwd.as_str())
                .with_env_var("APP_USER", conf.app_user.as_str())
                .with_env_var("APP_USER_PASSWORD", conf.app_user_pwd.as_str())
                .with_env_var("TABLE_PROVISIONER_USER", conf.table_provisioner_user.as_str())
                .with_env_var("TABLE_PROVISIONER_PASSWORD", conf.table_provisioner_pwd.as_str())
                .with_container_name(&id)
                .start()
                .await
                .must_expect("failed to start postgres-mae container");

            let port = container
                .get_host_port_ipv4(5432)
                .await
                .must_expect("failed to get postgres mapped port");

            Mutex::new(Some(Inner { container, id, port }))
        })
        .await
}

/// Open a fresh [`PgConnection`] with `search_path` set to a unique schema,
/// providing strong per-test isolation.
pub async fn spawn_scoped_schema() -> Result<PgScope> {
    let schema = format!("test_{}", Uuid::new_v4().to_string().replace('-', ""));
    let guard = pg_singleton().await.lock().await;
    let inner = guard.as_ref().ok_or_else(|| {
        anyhow::anyhow!("Postgres container not running — set MAE_TESTCONTAINERS=1")
    })?;

    let url = env::load().app_database_url_with_port(inner.port);
    let mut conn = PgConnection::connect(&url).await.context("failed to connect to Postgres")?;
    conn.execute(format!(r#"SET search_path TO "{}""#, schema).as_str()).await?;
    Ok(PgScope { schema, conn })
}

/// Stop the Postgres container and reset the singleton.
pub async fn teardown() {
    if let Some(m) = SINGLETON.get() {
        let mut guard = m.lock().await;
        if let Some(inner) = guard.take() {
            drop(inner);
        }
    }
}

/// Shared [`PgPool`] across all tests in the same process, initialised lazily.
pub(crate) async fn shared_pool() -> Result<&'static PgPool> {
    let pool = POOL
        .get_or_init(|| async {
            let init: Result<PgPool> = async {
                let guard = pg_singleton().await.lock().await;
                let inner = guard
                    .as_ref()
                    .context("Postgres container not running — set MAE_TESTCONTAINERS=1")?;

                run_premigration(inner).await.context("pre-migration script failed")?;

                let url = env::load().app_database_url_with_port(inner.port);
                PgPool::connect(&url).await.context("failed to connect to postgres")
            }
            .await;

            init.must_expect("failed to initialise postgres pool")
        })
        .await;

    Ok(pool)
}

/// Shared pool wrapped in `Arc` — convenience for building test contexts.
pub fn shared_pool_arc() -> Option<Arc<PgPool>> {
    POOL.get().map(|p| Arc::new(p.clone()))
}

// ── Pre-migration script ──────────────────────────────────────────────────────

async fn run_premigration(inner: &Inner) -> Result<()> {
    let script_path = "./scripts/sqlx_premigration.sh";

    let stdout_is_tty = std::io::stdout().is_terminal();
    let stderr_is_tty = std::io::stderr().is_terminal();

    let mut cmd = Command::new("bash");
    cmd.arg(script_path);

    let port =
        inner.container.get_host_port_ipv4(5432).await.context("failed to get container port")?;

    let e = env::load();

    cmd.env("NO_DOT_ENV", "1")
        .env("CONTAINER", &inner.id)
        .env("DEBUG", "1")
        .env("RUN_APP_MIGRATIONS", "1")
        .env("TTY_OVERRIDE", "1")
        .env("ADMIN_MIGRATIONS_PATH", &e.admin_migrations_path)
        .env("APP_MIGRATIONS_PATH", &e.app_migrations_path)
        .env("DB_HOST", &e.db_host)
        .env("DB_PORT", port.to_string())
        .env("APP_DB_NAME", &e.app_db_name)
        .env("SUPER_USER", &e.superuser)
        .env("SUPERUSER_PWD", &e.superuser_pwd)
        .env("MIGRATOR_USER", &e.migrator_user)
        .env("MIGRATOR_PWD", &e.migrator_pwd)
        .env("APP_USER", &e.app_user)
        .env("APP_USER_PWD", &e.app_user_pwd)
        .env("TABLE_PROVISIONER_USER", &e.table_provisioner_user)
        .env("TABLE_PROVISIONER_PWD", &e.table_provisioner_pwd)
        .env("SUPER_DATABASE_URL", e.super_database_url_with_port(port))
        .env("SEARCH_PATH", &e.search_path)
        .env("DATABASE_URL", e.database_url_with_port(port))
        .env("APP_DATABASE_URL", e.app_database_url_with_port(port))
        .env("TABLE_CREATOR_DATABASE_URL", e.table_creator_database_url_with_port(port));

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

    let output = cmd.output().await.with_context(|| format!("failed to spawn {script_path}"))?;

    if !output.status.success() {
        if !stdout_is_tty && !output.stdout.is_empty() {
            eprintln!("--- premigration stdout ---\n{}", String::from_utf8_lossy(&output.stdout));
        }
        if !stderr_is_tty && !output.stderr.is_empty() {
            eprintln!("--- premigration stderr ---\n{}", String::from_utf8_lossy(&output.stderr));
        }
        anyhow::bail!("premigration script exited with status: {}", output.status);
    }

    Ok(())
}
