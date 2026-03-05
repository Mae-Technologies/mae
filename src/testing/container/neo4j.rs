//! Neo4j container singleton + per-test database isolation.
//!
//! # Status — issue #40
//! Container spinup is implemented in issue #40.
//!
//! # Isolation strategy
//! Neo4j Enterprise supports named databases.  Each scope receives a unique
//! database (`test_<uuid>`).  Community edition falls back to a label prefix.

use anyhow::Result;
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

use super::MaeContainer;

// ── Singleton placeholder ─────────────────────────────────────────────────────

pub struct Inner {
    // Populated by #40:
    //   pub container: ContainerAsync<GenericImage>,
    //   pub id: String,
    #[allow(dead_code)]
    pub port: u16,
}

static SINGLETON: OnceCell<Mutex<Option<Inner,>,>,> = OnceCell::const_new();

// ── Public isolation scope ────────────────────────────────────────────────────

/// Per-test isolation scope for Neo4j.
pub struct Neo4jScope {
    /// Unique database name (Enterprise) or label prefix (Community).
    pub database: String,
}

impl Neo4jScope {
    /// Drop this database or delete all prefixed nodes.
    ///
    /// # Note — implemented in #40.
    pub async fn drop_database(&self,) -> Result<(),> {
        // TODO (#40): DROP DATABASE <database> IF EXISTS via Bolt.
        Ok((),)
    }
}

// ── MaeContainer impl ─────────────────────────────────────────────────────────

/// Zero-sized handle for the Neo4j container singleton.
pub struct Neo4jContainer;

impl MaeContainer for Neo4jContainer {
    type Scope = Neo4jScope;

    async fn start() -> Option<(),> {
        neo4j_singleton().await.lock().await.as_ref().map(|_| (),)
    }

    async fn scope() -> Result<Neo4jScope,> {
        spawn_scoped_database().await
    }

    async fn teardown() {
        teardown().await;
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Returns the Neo4j singleton (stub until #40).
pub async fn neo4j_singleton() -> &'static Mutex<Option<Inner,>,> {
    SINGLETON
        .get_or_init(|| async {
            // TODO (#40): start Neo4j testcontainer when MAE_TESTCONTAINERS=1.
            Mutex::new(None,)
        },)
        .await
}

/// Create a fresh per-test database scope.
///
/// # Note — implemented in #40.
pub async fn spawn_scoped_database() -> Result<Neo4jScope,> {
    let guard = neo4j_singleton().await.lock().await;
    let _inner = guard.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Neo4j container not running — set MAE_TESTCONTAINERS=1 (spinup lands in #40)"
        )
    },)?;

    // TODO (#40): CREATE DATABASE test_<uuid> via Bolt, then return scope.
    let database = format!("test_{}", Uuid::new_v4().to_string().replace('-', ""));
    Ok(Neo4jScope { database, },)
}

/// Stop the Neo4j container and reset the singleton.
pub async fn teardown() {
    if let Some(m,) = SINGLETON.get() {
        let mut guard = m.lock().await;
        let _ = guard.take();
    }
}
