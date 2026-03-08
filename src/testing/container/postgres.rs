//! Postgres container singleton + per-test schema isolation.

use crate::testing::{env, must::MustExpect};
use anyhow::{Context, Result, bail};
use sqlx::{Connection, Executor, PgConnection, PgPool};
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

#[derive(Clone, Debug)]
struct RunningPostgresConfig {
    host: String,
    port: u16,
    db_name: String,
    app_user: String,
    app_password: String,
    migrator_user: String,
    migrator_password: String,
    search_path: String
}

impl RunningPostgresConfig {
    fn app_database_url(&self) -> String {
        build_pg_url(
            &self.app_user,
            &self.app_password,
            &self.host,
            self.port,
            &self.db_name,
            Some(&self.search_path)
        )
    }

    fn migrator_database_url(&self) -> String {
        build_pg_url(
            &self.migrator_user,
            &self.migrator_password,
            &self.host,
            self.port,
            &self.db_name,
            Some(&self.search_path)
        )
    }
}

static SINGLETON: OnceCell<Mutex<Option<Inner>>> = OnceCell::const_new();
static POOL: OnceCell<PgPool> = OnceCell::const_new();
static FALLBACK_CONFIG: OnceCell<RunningPostgresConfig> = OnceCell::const_new();

pub struct PgScope {
    pub schema: String,
    pub conn: PgConnection
}

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
                .with_wait_for(WaitFor::message_on_stdout("pgTAP tests passed"));

            let container: ContainerAsync<GenericImage> = image
                .with_env_var("APP_DB_NAME", conf.app_db_name.as_str())
                .with_env_var("APP_ENV", "test")
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

pub async fn spawn_scoped_schema() -> Result<PgScope> {
    let schema = format!("test_{}", Uuid::new_v4().to_string().replace('-', ""));
    let url = connection_url().await?;

    let mut conn = PgConnection::connect(&url).await.context("failed to connect to Postgres")?;
    conn.execute(format!(r#"SET search_path TO "{}""#, schema).as_str()).await?;
    Ok(PgScope { schema, conn })
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
                    let url = env::load().app_database_url_with_port(inner.port);
                    return PgPool::connect(&url).await.context("failed to connect to postgres");
                }

                let conf = fallback_config()
                    .await
                    .context("Postgres fallback mode requires a running postgres instance")?;
                run_premigration_fallback(conf).await.context("pre-migration script failed")?;
                PgPool::connect(&conf.app_database_url())
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

async fn run_premigration_fallback(conf: &RunningPostgresConfig) -> Result<()> {
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

async fn connection_url() -> Result<String> {
    if let Some(inner) = pg_singleton().await.lock().await.as_ref() {
        return Ok(env::load().app_database_url_with_port(inner.port));
    }

    let conf = fallback_config().await.context(
        "Postgres container not running and fallback config is invalid. \
         Set MAE_TESTCONTAINERS=1 or provide test-safe fallback postgres settings"
    )?;

    Ok(conf.app_database_url())
}

async fn fallback_config() -> Result<&'static RunningPostgresConfig> {
    FALLBACK_CONFIG.get_or_try_init(|| async { fallback_config_from_env() }).await
}

fn fallback_config_from_env() -> Result<RunningPostgresConfig> {
    fallback_config_from_lookup(|k| std::env::var(k).ok())
}

fn fallback_config_from_lookup<F>(lookup: F) -> Result<RunningPostgresConfig>
where
    F: Fn(&str) -> Option<String>
{
    let host = lookup_or_default(&lookup, &["MAE_TEST_PG_HOST", "DB_HOST"], "127.0.0.1");
    let port = lookup_or_default(&lookup, &["MAE_TEST_PG_PORT", "DB_PORT"], "5432")
        .parse::<u16>()
        .context("MAE_TEST_PG_PORT/DB_PORT must be a valid u16")?;

    let db_name = lookup_or_default(&lookup, &["MAE_TEST_PG_DB", "APP_DB_NAME"], "mae_test");
    ensure_test_db_name(&db_name)?;

    let app_user = lookup_or_default(&lookup, &["MAE_TEST_PG_USER", "APP_USER"], "app");
    let app_password =
        lookup_or_default(&lookup, &["MAE_TEST_PG_PASSWORD", "APP_USER_PWD"], "secret");

    let migrator_user =
        lookup_or_default(&lookup, &["MAE_TEST_PG_MIGRATOR_USER", "MIGRATOR_USER"], "db_migrator");
    let migrator_password = lookup_or_default(
        &lookup,
        &["MAE_TEST_PG_MIGRATOR_PASSWORD", "MIGRATOR_PWD"],
        "migrator_secret"
    );

    let search_path = lookup_or_default(
        &lookup,
        &["MAE_TEST_PG_SEARCH_PATH", "SEARCH_PATH"],
        "options=-csearch_path%3Dapp"
    );

    Ok(RunningPostgresConfig {
        host,
        port,
        db_name,
        app_user,
        app_password,
        migrator_user,
        migrator_password,
        search_path
    })
}

fn lookup_or_default<F>(lookup: &F, keys: &[&str], default: &str) -> String
where
    F: Fn(&str) -> Option<String>
{
    keys.iter()
        .find_map(|k| lookup(k).filter(|v| !v.trim().is_empty()))
        .unwrap_or_else(|| default.to_owned())
}

fn ensure_test_db_name(db_name: &str) -> Result<()> {
    if !db_name.contains("_test") {
        bail!("Refusing to run against non-test database: '{db_name}'");
    }
    Ok(())
}

fn build_pg_url(
    user: &str,
    password: &str,
    host: &str,
    port: u16,
    db_name: &str,
    search_path: Option<&str>
) -> String {
    let mut url = format!("postgres://{user}:{password}@{host}:{port}/{db_name}");
    if let Some(search_path) = search_path {
        url.push('?');
        url.push_str(search_path);
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_config_uses_safe_defaults() {
        let cfg = fallback_config_from_lookup(|_| None).expect("expected defaults to parse");
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 5432);
        assert_eq!(cfg.db_name, "mae_test");
        assert!(cfg.app_database_url().contains("/mae_test"));
    }

    #[test]
    fn fallback_config_respects_overrides() {
        let cfg = fallback_config_from_lookup(|key| match key {
            "MAE_TEST_PG_HOST" => Some("localhost".to_owned()),
            "MAE_TEST_PG_PORT" => Some("6543".to_owned()),
            "MAE_TEST_PG_DB" => Some("custom_test".to_owned()),
            "MAE_TEST_PG_USER" => Some("alice".to_owned()),
            "MAE_TEST_PG_PASSWORD" => Some("pw".to_owned()),
            _ => None
        })
        .expect("expected overrides to parse");

        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 6543);
        assert_eq!(cfg.db_name, "custom_test");
        assert!(cfg.app_database_url().contains("postgres://alice:pw@localhost:6543/custom_test"));
    }

    #[test]
    fn fallback_rejects_non_test_database() {
        let err = fallback_config_from_lookup(|key| {
            if key == "MAE_TEST_PG_DB" { Some("mae".to_owned()) } else { None }
        })
        .expect_err("non-test db must fail");

        assert!(format!("{err:#}").contains("Refusing to run against non-test database"));
    }
}
