//! Singleton Neo4j testcontainer with per-test label-namespace isolation.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use mae::testing::containers::neo4j::{get_neo4j_bolt, IsolatedNeo4j};
//!
//! #[tokio::test]
//! async fn my_test() {
//!     let (url, port) = get_neo4j_bolt().await.expect("neo4j must be running");
//!     let ns = IsolatedNeo4j::new();  // unique label prefix for this test
//!     // Use ns.prefix() to namespace all node labels in your queries.
//! }
//! ```
//!
//! ## Per-test isolation
//!
//! Neo4j Community Edition supports only the built-in `neo4j` and `system`
//! databases, so we cannot create per-test databases.  Instead each test
//! receives a unique label **prefix** via [`IsolatedNeo4j`].  Include the
//! prefix in every node label you create and match, e.g.:
//!
//! ```cypher
//! CREATE (n:test_a3f2__User { name: "Alice" })
//! MATCH (n:test_a3f2__User) RETURN n
//! ```
//!
//! After the test, call [`IsolatedNeo4j::cleanup`] to delete all nodes
//! carrying the prefix label.

use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::neo4j::{Neo4j, Neo4jImage};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::testing::must::testcontainers_enabled;

// ── Singleton state ──────────────────────────────────────────────────────────

/// Internal container state.
pub struct Neo4jContainer {
    pub container: ContainerAsync<Neo4jImage,>,
    pub bolt_port: u16,
    pub bolt_url: String,
}

/// The singleton mutex.  Always present; `None` when no container is running.
static NEO4J: OnceLock<Mutex<Option<Neo4jContainer,>,>,> = OnceLock::new();

fn neo4j_mutex() -> &'static Mutex<Option<Neo4jContainer,>,> {
    NEO4J.get_or_init(|| Mutex::new(None,),)
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Returns `(bolt_url, bolt_port)` for the shared Neo4j container.
///
/// Starts the container on the first call (or after a [`teardown`]).
///
/// # Errors
/// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled or the
/// container fails to start.
pub async fn get_neo4j_bolt() -> Result<(String, u16,),> {
    ensure_started().await?;

    let guard = neo4j_mutex().lock().await;
    let c = guard.as_ref().context("neo4j container is not running — this is a bug",)?;
    Ok((c.bolt_url.clone(), c.bolt_port,),)
}

/// Stops the Neo4j container and resets the singleton.
///
/// The next call to [`get_neo4j_bolt`] will start a fresh container.
pub async fn teardown() {
    let mut guard = neo4j_mutex().lock().await;
    if let Some(c,) = guard.take() {
        // Explicit stop before drop for a clean shutdown.
        let _ = c.container.stop().await;
        drop(c,);
    }
}

// ── Per-test isolation ───────────────────────────────────────────────────────

/// A unique label prefix for one test's nodes.
///
/// Use [`IsolatedNeo4j::prefix`] to namespace every node label you create.
/// Call [`IsolatedNeo4j::cleanup`] at the end of the test to delete all
/// nodes belonging to this test.
pub struct IsolatedNeo4j {
    prefix: String,
}

impl IsolatedNeo4j {
    /// Creates a new isolation handle with a unique prefix.
    pub fn new() -> Self {
        let id = Uuid::new_v4().to_string().replace('-', "",)[..8].to_string();
        Self { prefix: format!("mae_test_{id}"), }
    }

    /// Returns the label prefix for this test, e.g. `"mae_test_a3f2b1c8"`.
    pub fn prefix(&self,) -> &str {
        &self.prefix
    }

    /// Returns a namespaced label, e.g. `"mae_test_a3f2b1c8__User"`.
    pub fn label(&self, base: &str,) -> String {
        format!("{}_{base}", self.prefix)
    }

    /// Deletes all nodes in the database that carry the isolation prefix label.
    ///
    /// Requires a live Neo4j driver connection; pass the `bolt_url` from
    /// [`get_neo4j_bolt`].  The cleanup query is:
    ///
    /// ```cypher
    /// MATCH (n) WHERE any(l IN labels(n) WHERE l STARTS WITH $prefix)
    /// DETACH DELETE n
    /// ```
    pub fn cleanup_query(&self,) -> (String, String,) {
        (
            "MATCH (n) WHERE any(l IN labels(n) WHERE l STARTS WITH $prefix) DETACH DELETE n"
                .to_string(),
            self.prefix.clone(),
        )
    }
}

impl Default for IsolatedNeo4j {
    fn default() -> Self {
        Self::new()
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

async fn ensure_started() -> Result<(),> {
    if !testcontainers_enabled() {
        bail!(
            "testcontainers disabled — set MAE_TESTCONTAINERS=1 to run Neo4j tests. \
             In Concourse, ensure the task has Docker socket access (DinD)."
        );
    }

    let mut guard = neo4j_mutex().lock().await;
    if guard.is_none() {
        let container: ContainerAsync<Neo4jImage,> =
            Neo4j::new().start().await.context("failed to start Neo4j container",)?;

        let bolt_port = container
            .image()
            .bolt_port_ipv4()
            .context("failed to get Neo4j bolt port from image",)?;

        let bolt_url = format!("bolt://localhost:{bolt_port}");

        *guard = Some(Neo4jContainer { container, bolt_port, bolt_url, },);
    }
    Ok((),)
}
