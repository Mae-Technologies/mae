//! Neo4j container singleton + per-test label-namespace isolation.
//!
//! # Isolation strategy
//! Neo4j Community Edition supports only the built-in `neo4j` and `system`
//! databases, so we cannot create per-test databases.  Instead each test
//! receives a unique label **prefix** via [`Neo4jScope`].  Include the
//! prefix in every node label you create and match, e.g.:
//!
//! ```cypher
//! CREATE (n:test_a3f2__User { name: "Alice" })
//! MATCH (n:test_a3f2__User) RETURN n
//! ```
//!
//! After the test, call the cleanup query from [`Neo4jScope::cleanup_query`]
//! to delete all nodes carrying the prefix label.

use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::neo4j::{Neo4j, Neo4jImage};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::testing::must::testcontainers_enabled;

use super::MaeContainer;

// ── Singleton state ───────────────────────────────────────────────────────────

pub struct Inner {
    pub container: ContainerAsync<Neo4jImage>,
    pub bolt_port: u16,
    pub bolt_url: String
}

static SINGLETON: OnceLock<Mutex<Option<Inner>>> = OnceLock::new();

fn neo4j_mutex() -> &'static Mutex<Option<Inner>> {
    SINGLETON.get_or_init(|| Mutex::new(None))
}

// ── Public isolation scope ────────────────────────────────────────────────────

/// Per-test isolation scope for Neo4j.
pub struct Neo4jScope {
    /// Unique label prefix (e.g. `mae_test_a3f2b1c8`).
    pub database: String
}

impl Neo4jScope {
    /// Returns the cleanup Cypher query and the prefix string.
    ///
    /// Run `MATCH (n) WHERE any(l IN labels(n) WHERE l STARTS WITH $prefix) DETACH DELETE n`
    /// with parameter `prefix = self.database` to remove all nodes belonging to this test.
    pub fn cleanup_query(&self) -> (String, String) {
        (
            "MATCH (n) WHERE any(l IN labels(n) WHERE l STARTS WITH $prefix) DETACH DELETE n"
                .to_string(),
            self.database.clone()
        )
    }

    /// Returns a namespaced label, e.g. `"mae_test_a3f2b1c8__User"`.
    pub fn label(&self, base: &str) -> String {
        format!("{}_{base}", self.database)
    }

    /// Drop (clean up) this scope's nodes.
    ///
    /// Caller must issue the [`cleanup_query`](Self::cleanup_query) against
    /// a live Neo4j Bolt connection; this method is a no-op stub suitable
    /// for contexts where no driver is available.
    pub async fn drop_database(&self) -> Result<()> {
        // Cleanup is performed via the Bolt driver by the caller using cleanup_query().
        Ok(())
    }
}

// ── MaeContainer impl ─────────────────────────────────────────────────────────

/// Zero-sized handle for the Neo4j container singleton.
pub struct Neo4jContainer;

impl MaeContainer for Neo4jContainer {
    type Scope = Neo4jScope;

    async fn start() -> Option<()> {
        neo4j_singleton().await.lock().await.as_ref().map(|_| ())
    }

    async fn scope() -> Result<Neo4jScope> {
        spawn_scoped_database().await
    }

    async fn teardown() {
        teardown().await;
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Returns `(bolt_url, bolt_port)` for the shared Neo4j container.
///
/// Starts the container on the first call (or after a [`teardown`]).
///
/// # Errors
/// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled or the container
/// fails to start.
pub async fn get_neo4j_bolt() -> Result<(String, u16)> {
    ensure_started().await?;
    let guard = neo4j_mutex().lock().await;
    let c = guard.as_ref().context("neo4j container is not running — this is a bug")?;
    Ok((c.bolt_url.clone(), c.bolt_port))
}

/// Returns the Neo4j singleton, starting the container if needed.
pub async fn neo4j_singleton() -> &'static Mutex<Option<Inner>> {
    // Ensure the container is started (ignoring errors — callers that need the
    // container should use get_neo4j_bolt / spawn_scoped_database directly).
    let _ = ensure_started().await;
    neo4j_mutex()
}

/// Create a fresh per-test label-namespace scope.
///
/// # Errors
/// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled.
pub async fn spawn_scoped_database() -> Result<Neo4jScope> {
    ensure_started().await?;
    let guard = neo4j_mutex().lock().await;
    let _inner = guard.as_ref().context("Neo4j container not running — this is a bug")?;
    let id = Uuid::new_v4().to_string().replace('-', "")[..8].to_string();
    Ok(Neo4jScope { database: format!("mae_test_{id}") })
}

/// Stop the Neo4j container and reset the singleton.
pub async fn teardown() {
    let mut guard = neo4j_mutex().lock().await;
    if let Some(c) = guard.take() {
        let _ = c.container.stop().await;
        drop(c);
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn ensure_started() -> Result<()> {
    if !testcontainers_enabled() {
        bail!(
            "testcontainers disabled — set MAE_TESTCONTAINERS=1 to run Neo4j tests. \
             In Concourse, ensure the task has Docker socket access (DinD)."
        );
    }

    let mut guard = neo4j_mutex().lock().await;
    if guard.is_none() {
        let container: ContainerAsync<Neo4jImage> =
            Neo4j::new().with_password("testpassword").start().await.context("failed to start Neo4j container")?;

        let bolt_port = container
            .image()
            .bolt_port_ipv4()
            .context("failed to get Neo4j bolt port from image")?;

        let bolt_url = format!("bolt://localhost:{bolt_port}");

        *guard = Some(Inner { container, bolt_port, bolt_url });
    }
    Ok(())
}
