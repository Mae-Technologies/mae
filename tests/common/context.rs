use super::{env, must::MustExpect};
use anyhow::{Context, Result};
use mae::request_context::RequestContext;
use mae::session::Session;
use sqlx::PgPool;
use sqlx::{Connection, Executor, PgConnection};
use std::sync::Arc;
use testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

#[derive(Clone)]
pub struct TestContext {}

impl TestContext {
    pub async fn scoped_connection(&self) -> Result<PgConnection> {
        let schema = format!("test_{}", Uuid::new_v4().to_string().replace('-', ""));
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
    pub container: ContainerAsync<GenericImage>,
    pub id: String,
    pub port: u16
}

static PG_CONTAINER: OnceCell<Mutex<Option<TestContainer>>> = OnceCell::const_new();

async fn pg_container() -> &'static Mutex<Option<TestContainer>> {
    PG_CONTAINER
        .get_or_init(|| async {
            let id = format!("test_container_{}", Uuid::new_v4().to_string().replace('-', ""));
            let enabled = std::env::var("MAE_TESTCONTAINERS")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            let _conf = env::load();

            if !enabled {
                return Mutex::new(None);
            }
            let conf = env::load();

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
            Mutex::new(Some(TestContainer { container, id, port }))
        })
        .await
}

pub async fn teardown() {
    if let Some(m) = PG_CONTAINER.get() {
        let mut guard = m.lock().await;

        if let Some(container) = guard.take() {
            drop(container);
        }
    }
}

pub async fn get_context() -> Result<RequestContext<TestContext>> {
    let pool = Arc::new(pool().await?.clone());

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
            let init: Result<PgPool> = async {
                let m = pg_container().await;
                let guard = m.lock().await;

                let c = guard.as_ref().context(
                    "testcontainers disabled. Set MAE_TESTCONTAINERS=1 to run DB tests."
                )?;

                run_premigration(c).await.context("pre-migrations failed")?;

                let app_url = &env::load().app_database_url_with_port(c.port);
                let pool =
                    PgPool::connect(app_url).await.context("failed to connect to postgres")?;

                Ok(pool)
            }
            .await;

            init.must_expect("failed to initialize postgres pool")
        })
        .await;

    Ok(pool_ref)
}

async fn run_premigration(container: &TestContainer) -> Result<()> {
    let port = container.container.get_host_port_ipv4(5432).await.context("failed to get port")?;
    let cfg = env::load();

    let migrator_url = cfg.database_url_with_port(port);
    let migrator_pool =
        PgPool::connect(&migrator_url).await.context("failed to connect migrator pool")?;

    sqlx::migrate!("./migrations")
        .run(&migrator_pool)
        .await
        .context("service migrations failed")?;

    migrator_pool.close().await;

    Ok(())
}

#[cfg(test)]
mod test_context {
    use crate::common::must::must_eq;
    use sqlx::Row;

    use super::get_context;
    use anyhow::Result;
    use mae_macros::mae_test;

    #[cfg_attr(miri, ignore)]
    #[mae_test]
    async fn parallelism() -> Result<()> {
        let ctx = get_context().await?;
        let mut conn = ctx.custom.scoped_connection().await?;

        let n: i32 = sqlx::query("SELECT 1").fetch_one(&mut conn).await?.get(0);
        must_eq(n, 1);

        Ok(())
    }

    #[cfg_attr(miri, ignore)]
    #[mae_test]
    async fn uses_test_context_schema_isolation() -> Result<()> {
        let ctx = get_context().await?;
        let mut conn = ctx.custom.scoped_connection().await?;

        sqlx::query("SELECT 1").execute(&mut conn).await?;

        Ok(())
    }
}
