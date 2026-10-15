//! Grant snapshot round trip against the shared Redis container.
//!
//! Run with:
//!   MAE_TESTCONTAINERS=1 cargo test --features integration-testing --test grants

#![cfg(feature = "integration-testing")]

use anyhow::Result;
use mae::context::{ContextAccessor, RequestContext};
use mae::grants::{DEFAULT_TTL, GrantConfig, GrantStore, GrantWriter, GrantsError, key};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CompanyScope {
    sys_client: Option<Vec<i32>>
}
use mae::middleware::{MicroserviceAuth, get_microservice_session};
use mae::testing::must::{must_be_true, must_eq};
use mae_macros::mae_test;

#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn grant_snapshot_round_trip() -> Result<()> {
    let scope = mae::testing::container::redis::spawn_scoped_keyspace().await?;
    let writer = GrantWriter::connect(scope.url()).await?;
    let store = GrantStore::connect(scope.url()).await?;

    let snapshot = CompanyScope { sys_client: Some(vec![4, 3, 3]) };
    writer.put(7, &snapshot).await?;
    let loaded =
        store.get::<CompanyScope>(7).await?.ok_or_else(|| anyhow::anyhow!("missing snapshot"))?;
    must_eq(loaded, snapshot);

    writer.put(8, &CompanyScope { sys_client: Some(vec![]) }).await?;
    let nobody = store
        .get::<CompanyScope>(8)
        .await?
        .ok_or_else(|| anyhow::anyhow!("missing empty snapshot"))?;
    must_eq(nobody.sys_client, Some(vec![]));

    let client = redis::Client::open(scope.url())?;
    let mut conn = client.get_multiplexed_async_connection().await?;
    let ttl: i64 = redis::cmd("TTL").arg(key(7)).query_async(&mut conn).await?;
    must_be_true(ttl > 0 && ttl as u64 <= DEFAULT_TTL.as_secs());

    redis::cmd("SET").arg(key(9)).arg("not-json").query_async::<()>(&mut conn).await?;
    must_eq(store.get::<CompanyScope>(9).await, Err(GrantsError::Unreadable));

    writer.delete(7).await?;
    must_be_true(store.get::<CompanyScope>(7).await?.is_none());
    Ok(())
}

#[derive(Clone)]
struct AppCtx {
    store: GrantStore
}

impl MicroserviceAuth for AppCtx {
    fn micro_service_key(&self) -> &str {
        "x-service-key"
    }
    fn micro_service_pass(&self) -> &str {
        "secret"
    }
    fn grant_store(&self) -> Option<&GrantStore> {
        Some(&self.store)
    }
}

impl GrantConfig for AppCtx {
    type Snapshot = CompanyScope;
}

async fn listed_companies(ctx: RequestContext<AppCtx>) -> actix_web::HttpResponse {
    let companies = ctx.session().roles.snapshot().and_then(|row| row.sys_client.clone());
    actix_web::HttpResponse::Ok().json(companies)
}

async fn stored_snapshot(url: &str, user_id: i32) -> Result<Option<String>> {
    let client = redis::Client::open(url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;
    let raw: Option<String> = redis::cmd("GET").arg(key(user_id)).query_async(&mut conn).await?;
    Ok(raw)
}

/// Middleware copies the Redis snapshot onto the request context.
/// The stored document is the same after the request.
#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn middleware_copies_the_snapshot_and_does_not_write() -> Result<()> {
    let scope = mae::testing::container::redis::spawn_scoped_keyspace().await?;
    let writer = GrantWriter::connect(scope.url()).await?;
    writer.put(7, &CompanyScope { sys_client: Some(vec![3, 4]) }).await?;
    let before = stored_snapshot(scope.url(), 7).await?;

    let pool = sqlx::PgPool::connect_lazy("postgres://fake:fake@127.0.0.1:9/fake")?;
    let app_ctx = AppCtx { store: GrantStore::connect(scope.url()).await? };
    let app = actix_web::test::init_service(
        actix_web::App::new()
            .app_data(actix_web::web::Data::new(pool))
            .app_data(actix_web::web::Data::new(app_ctx))
            .wrap(actix_web::middleware::from_fn(get_microservice_session::<AppCtx>))
            .route("/companies", actix_web::web::get().to(listed_companies))
    )
    .await;

    let allowed = actix_web::test::TestRequest::get()
        .uri("/companies")
        .insert_header(("x-service-key", "secret"))
        .insert_header(("X-Session-User", "7"))
        .to_request();
    let resp = actix_web::test::try_call_service(&app, allowed)
        .await
        .map_err(|err| anyhow::anyhow!("allowed call failed: {err}"))?;
    must_eq(resp.status().as_u16(), 200);
    let body = actix_web::test::read_body(resp).await;
    let companies: Option<Vec<i32>> = serde_json::from_slice(&body)?;
    must_eq(companies, Some(vec![3, 4]));
    must_eq(stored_snapshot(scope.url(), 7).await?, before.clone());

    let missing = actix_web::test::TestRequest::get()
        .uri("/companies")
        .insert_header(("x-service-key", "secret"))
        .insert_header(("X-Session-User", "8"))
        .to_request();
    let denied = actix_web::test::try_call_service(&app, missing)
        .await
        .err()
        .ok_or_else(|| anyhow::anyhow!("user with no snapshot was allowed"))?;
    must_eq(denied.error_response().status().as_u16(), 401);
    must_be_true(stored_snapshot(scope.url(), 8).await?.is_none());

    let internal = actix_web::test::TestRequest::get()
        .uri("/companies")
        .insert_header(("x-service-key", "secret"))
        .to_request();
    let open = actix_web::test::try_call_service(&app, internal)
        .await
        .map_err(|err| anyhow::anyhow!("call without a user failed: {err}"))?;
    must_eq(open.status().as_u16(), 200);
    let open_body = actix_web::test::read_body(open).await;
    let unrestricted: Option<Vec<i32>> = serde_json::from_slice(&open_body)?;
    must_be_true(unrestricted.is_none());
    must_eq(stored_snapshot(scope.url(), 7).await?, before);
    Ok(())
}
