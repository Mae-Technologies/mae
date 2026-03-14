//! Integration smoke test for GraphDatabaseSettings::connect().
//!
//! Gated behind the `integration-testing` feature flag and the Docker flag
//! (`MAE_TESTCONTAINERS=1` at compile time).
//!
//! Run with:
//!   MAE_TESTCONTAINERS=1 cargo test --features integration-testing --test graphdb_settings

#![cfg(feature = "integration-testing")]

use anyhow::Result;
use mae::app::configuration::GraphDatabaseSettings;
use mae::testing::must::must_eq;
use mae_macros::mae_test;

#[mae_test(docker, teardown = mae::testing::container::teardown_all)]
async fn graphdb_settings_connect_returns_live_graph() -> Result<()> {
    let (bolt_url, bolt_port) = mae::testing::container::neo4j::get_neo4j_bolt().await?;

    let host = bolt_url
        .trim_start_matches("bolt://")
        .rsplit_once(':')
        .map(|(h, _)| h.to_string())
        .ok_or_else(|| anyhow::anyhow!("unexpected bolt URL format: {bolt_url}"))?;

    // Load credentials (username/password) from YAML config.
    // Override host/port with the live container's actual values.
    let mut settings = GraphDatabaseSettings::from_config()
        .map_err(|e| anyhow::anyhow!("failed to load graphdb config: {e}"))?;
    settings.host = host;
    settings.port = bolt_port;

    let graph = settings.connect().await?;

    let mut result = graph.execute(neo4rs::query("RETURN 1 AS n")).await?;

    let row = result
        .next()
        .await?
        .ok_or_else(|| anyhow::anyhow!("expected one row from RETURN 1 AS n"))?;

    let n: i64 = row.get("n")?;
    must_eq(n, 1_i64);

    Ok(())
}
