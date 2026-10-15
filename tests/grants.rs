//! Grant snapshot round trip against the shared Redis container.
//!
//! Run with:
//!   MAE_TESTCONTAINERS=1 cargo test --features integration-testing --test grants

#![cfg(feature = "integration-testing")]

use anyhow::Result;
use mae::grants::{DEFAULT_TTL, GrantStore, Grants, GrantsError, key};
use mae::testing::must::{must_be_true, must_eq};
use mae_macros::mae_test;

#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn grant_snapshot_round_trip() -> Result<()> {
    let scope = mae::testing::container::redis::spawn_scoped_keyspace().await?;
    let store = GrantStore::connect(scope.url()).await?;

    store.put(7, &Grants { sys_client: Some(vec![4, 3, 3]) }).await?;
    let loaded = store.get(7).await?.ok_or_else(|| anyhow::anyhow!("missing snapshot"))?;
    must_eq(loaded, Grants::companies(vec![3, 4]));

    store.put(7, &Grants::open()).await?;
    let open = store.get(7).await?.ok_or_else(|| anyhow::anyhow!("missing open snapshot"))?;
    must_be_true(open.sys_clients().is_none());

    store.put(8, &Grants::companies(vec![])).await?;
    let nobody = store.get(8).await?.ok_or_else(|| anyhow::anyhow!("missing empty snapshot"))?;
    must_eq(nobody.sys_clients(), Some(vec![]));

    let client = redis::Client::open(scope.url())?;
    let mut conn = client.get_multiplexed_async_connection().await?;
    let ttl: i64 = redis::cmd("TTL").arg(key(7)).query_async(&mut conn).await?;
    must_be_true(ttl > 0 && ttl as u64 <= DEFAULT_TTL.as_secs());

    redis::cmd("SET").arg(key(9)).arg("not-json").query_async::<()>(&mut conn).await?;
    must_eq(store.get(9).await, Err(GrantsError::Unreadable));

    store.delete(7).await?;
    must_be_true(store.get(7).await?.is_none());
    Ok(())
}
