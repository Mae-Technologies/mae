//! Postgres container singleton for mae integration tests.
//!
//! Per-test isolation is achieved via transaction rollback (`pool.begin()` +
//! drop), not DDL-based schema creation.  This avoids requiring elevated
//! database permissions that hardened postgres-mae doesn't grant.

use crate::testing::{env, must::MustExpect};
use anyhow::{Context, Result, bail};
use sqlx::PgPool;
use std::sync::Arc;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

use super::MaeContainer;

pub struct Inner {
    pub container: ContainerAsync<GenericImage>,
    pub id: String,
    pub port: u16
}

static SINGLETON: OnceCell<Mutex<Option<Inner>>> = OnceCell::const_new();
static POOL: OnceCell<PgPool> = OnceCell::const_new();

pub struct PostgresContainer;

impl MaeContainer for PostgresContainer {
    type Scope = ();

    async fn start() -> Option<()> {
        pg_singleton().await.lock().await.as_ref().map(|_| ())
    }

    async fn scope() -> Result<()> {
        // Isolation is handled via transaction rollback at the test level.
        Ok(())
    }

    async fn teardown() {
        teardown().await;
    }
}

pub async fn pg_singleton() -> &'static Mutex<Option<Inner>> {
    SINGLETON
        .get_or_init(|| async {
            let enabled = std::env::var("MAE_TESTCONTAINERS")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            if !enabled {
                return Mutex::new(None);
            }

            let conf = env::load();
            let id = format!("mae_pg_{}", Uuid::new_v4().to_string().replace('-', ""));

            let image = GenericImage::new("ghcr.io/mae-technologies/postgres-mae", "latest")
                .with_exposed_port(5432.tcp())
                .with_wait_for(WaitFor::message_on_stdout("Premigration script finished"));

            let container: ContainerAsync<GenericImage> = image
                .with_env_var("APP_DB_NAME", conf.app_db_name.as_str())
                .with_env_var("APP_ENV", "dev")
                .with_env_var("CONFIRM_IRREVOCABLE_DATABASE_WIPE", "true")
                .with_env_var("SUPERUSER", conf.superuser.as_str())
                .with_env_var("SUPERUSER_PWD", conf.superuser_pwd.as_str())
                .with_env_var("SUPERUSER_DB", "postgres")
                .with_env_var("APP_USER", conf.app_user.as_str())
                .with_env_var("APP_USER_PWD", conf.app_user_pwd.as_str())
                .with_env_var("MIGRATOR_USER", conf.migrator_user.as_str())
                .with_env_var("MIGRATOR_PWD", conf.migrator_pwd.as_str())
                .with_env_var("TABLE_PROVISIONER_USER", conf.table_provisioner_user.as_str())
                .with_env_var("TABLE_PROVISIONER_PWD", conf.table_provisioner_pwd.as_str())
                .with_env_var("MAE_DB_NAME", "mae")
                .with_env_var("TEST_DB_NAME", "test_db")
                .with_env_var("DB_HOST", "127.0.0.1")
                .with_env_var("DB_PORT", "5432")
                .with_env_var("PG_TEST_LOG", "1")
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

pub async fn teardown() {
    if let Some(m) = SINGLETON.get() {
        let mut guard = m.lock().await;
        if let Some(inner) = guard.take() {
            drop(inner);
        }
    }
}

pub(crate) async fn shared_pool() -> Result<&'static PgPool> {
    let pool = POOL
        .get_or_init(|| async {
            let init: Result<PgPool> = async {
                if let Some(inner) = pg_singleton().await.lock().await.as_ref() {
                    run_premigration_container(inner)
                        .await
                        .context("pre-migration script failed")?;
                    let url = env::load().database_url_with_port(inner.port);
                    return PgPool::connect(&url).await.context("failed to connect to postgres");
                }

                let conf = env::load();
                ensure_test_db_name(&conf.app_db_name)?;
                run_premigration_fallback(conf).await.context("pre-migration script failed")?;
                PgPool::connect(&conf.migrator_database_url())
                    .await
                    .context("failed to connect to postgres fallback")
            }
            .await;

            init.must_expect("failed to initialise postgres pool")
        })
        .await;

    Ok(pool)
}

pub fn shared_pool_arc() -> Option<Arc<PgPool>> {
    POOL.get().map(|p| Arc::new(p.clone()))
}

async fn run_premigration_container(inner: &Inner) -> Result<()> {
    let port =
        inner.container.get_host_port_ipv4(5432).await.context("failed to get container port")?;

    let e = env::load();
    let migrator_url = e.database_url_with_port(port);
    run_migrations(&migrator_url).await
}

async fn run_premigration_fallback(conf: &env::DotEnv) -> Result<()> {
    run_migrations(&conf.migrator_database_url()).await
}

async fn run_migrations(migrator_url: &str) -> Result<()> {
    let migrator_pool =
        PgPool::connect(migrator_url).await.context("failed to connect migrator pool")?;

    sqlx::migrate!("./migrations")
        .run(&migrator_pool)
        .await
        .context("service migrations failed")?;

    migrator_pool.close().await;
    Ok(())
}

fn ensure_test_db_name(db_name: &str) -> Result<()> {
    if !db_name.contains("_test") {
        bail!("Refusing to run against non-test database: '{db_name}'");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_test_db_name_accepts_test_database() {
        ensure_test_db_name("mae_test").expect("_test db should pass");
    }

    #[test]
    fn ensure_test_db_name_rejects_non_test_database() {
        let err = ensure_test_db_name("mae").expect_err("non-test db must fail");
        assert!(format!("{err:#}").contains("Refusing to run against non-test database"));
    }
}
