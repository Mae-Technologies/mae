//! Redis container singleton + per-test keyspace isolation.
//!
//! # Status — issue #40
//! Container spinup is implemented in issue #40.  The trait impl, scope type,
//! and `teardown()` are defined here so consumers can import them today.
//!
//! # Isolation strategy
//! Each test scope selects a dedicated logical database (0–15) and holds a
//! unique key prefix.  `RedisScope::flush()` runs `FLUSHDB` on teardown.

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

/// Per-test isolation scope for Redis.
pub struct RedisScope {
    /// Logical Redis database index (0–15) assigned to this scope.
    pub db_index: u8,
    /// Key prefix — prepend to every key your test writes.
    pub key_prefix: String,
}

impl RedisScope {
    /// Flush all keys in this scope's database.
    ///
    /// # Note — implemented in #40.
    pub async fn flush(&self,) -> Result<(),> {
        // TODO (#40): SELECT db_index; FLUSHDB
        Ok((),)
    }
}

// ── MaeContainer impl ─────────────────────────────────────────────────────────

/// Zero-sized handle for the Redis container singleton.
pub struct RedisContainer;

impl MaeContainer for RedisContainer {
    type Scope = RedisScope;

    async fn start() -> Option<(),> {
        redis_singleton().await.lock().await.as_ref().map(|_| (),)
    }

    async fn scope() -> Result<RedisScope,> {
        spawn_scoped_keyspace().await
    }

    async fn teardown() {
        teardown().await;
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Returns the Redis singleton (stub until #40).
pub async fn redis_singleton() -> &'static Mutex<Option<Inner,>,> {
    SINGLETON
        .get_or_init(|| async {
            // TODO (#40): start Redis testcontainer when MAE_TESTCONTAINERS=1.
            Mutex::new(None,)
        },)
        .await
}

/// Create a fresh per-test keyspace scope.
///
/// # Note — implemented in #40.
pub async fn spawn_scoped_keyspace() -> Result<RedisScope,> {
    let guard = redis_singleton().await.lock().await;
    let _inner = guard.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "Redis container not running — set MAE_TESTCONTAINERS=1 (spinup lands in #40)"
        )
    },)?;

    // TODO (#40): use an atomic counter to hand out db indices 0–15.
    let db_index = 0u8;
    let key_prefix = format!("test:{}", Uuid::new_v4().to_string().replace('-', ""));
    Ok(RedisScope { db_index, key_prefix, },)
}

/// Stop the Redis container and reset the singleton.
pub async fn teardown() {
    if let Some(m,) = SINGLETON.get() {
        let mut guard = m.lock().await;
        let _ = guard.take();
    }
}
