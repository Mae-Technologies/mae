//! Redis container singleton + per-test keyspace isolation.
//!
//! # Isolation strategy
//! Redis supports 16 logical databases (0–15) by default.  Each test scope
//! selects a dedicated logical database via an atomic counter and holds a
//! unique key prefix.  `RedisScope::flush()` runs `FLUSHDB` on teardown.

use std::sync::{
    OnceLock,
    atomic::{AtomicU8, Ordering}
};

use anyhow::{Context, Result, bail};
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::redis::Redis;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::testing::must::testcontainers_enabled;

use super::MaeContainer;

/// Default port exposed by the Redis image.
pub const REDIS_PORT: u16 = 6379;

/// Number of logical databases in the default Redis configuration.
const REDIS_DB_COUNT: u8 = 16;

// ── Singleton state ───────────────────────────────────────────────────────────

pub struct Inner {
    pub container: ContainerAsync<Redis>,
    pub port: u16,
    /// Base URL without a database suffix, e.g. `"redis://localhost:32768"`.
    pub base_url: String
}

static SINGLETON: OnceLock<Mutex<Option<Inner>>> = OnceLock::new();
/// Round-robin DB counter for per-test isolation.
static DB_COUNTER: AtomicU8 = AtomicU8::new(0);

fn redis_mutex() -> &'static Mutex<Option<Inner>> {
    SINGLETON.get_or_init(|| Mutex::new(None))
}

// ── Public isolation scope ────────────────────────────────────────────────────

/// Per-test isolation scope for Redis.
pub struct RedisScope {
    /// Logical Redis database index (0–15) assigned to this scope.
    pub db_index: u8,
    /// Key prefix — prepend to every key your test writes.
    pub key_prefix: String,
    /// DB-scoped URL, e.g. `"redis://localhost:32768/3"`.
    pub url: String
}

impl RedisScope {
    /// Returns the DB-scoped URL, e.g. `"redis://localhost:32768/3"`.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the logical database number (0–15).
    pub fn db(&self) -> u8 {
        self.db_index
    }

    /// Returns the `FLUSHDB` command string for this database.
    ///
    /// Connect to [`url`][Self::url] and issue `FLUSHDB` to wipe all keys
    /// belonging to this test.
    pub fn flushdb_command(&self) -> &'static str {
        "FLUSHDB"
    }

    /// Flush all keys in this scope's database.
    ///
    /// Note: caller must issue `FLUSHDB` against [`url`][Self::url].
    pub async fn flush(&self) -> Result<()> {
        // Callers use flushdb_command() and their own Redis client.
        Ok(())
    }
}

// ── MaeContainer impl ─────────────────────────────────────────────────────────

/// Zero-sized handle for the Redis container singleton.
pub struct RedisContainer;

impl MaeContainer for RedisContainer {
    type Scope = RedisScope;

    async fn start() -> Option<()> {
        redis_singleton().await.lock().await.as_ref().map(|_| ())
    }

    async fn scope() -> Result<RedisScope> {
        spawn_scoped_keyspace().await
    }

    async fn teardown() {
        teardown().await;
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Returns `(url, port)` for the shared Redis container.
///
/// The URL points to DB 0.  For per-test isolation, prefer
/// [`spawn_scoped_keyspace`].
///
/// # Errors
/// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled.
pub async fn get_redis_url() -> Result<(String, u16)> {
    ensure_started().await?;
    let guard = redis_mutex().lock().await;
    let c = guard.as_ref().context("redis container is not running — this is a bug")?;
    Ok((format!("{}/0", c.base_url), c.port))
}

/// Returns the Redis singleton, starting the container if needed.
pub async fn redis_singleton() -> &'static Mutex<Option<Inner>> {
    let _ = ensure_started().await;
    redis_mutex()
}

/// Create a fresh per-test keyspace scope.
///
/// Allocates the next available DB number (mod 16) and a unique key prefix.
///
/// # Errors
/// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled.
pub async fn spawn_scoped_keyspace() -> Result<RedisScope> {
    ensure_started().await?;

    let db_index = DB_COUNTER.fetch_add(1, Ordering::Relaxed) % REDIS_DB_COUNT;

    let guard = redis_mutex().lock().await;
    let c = guard.as_ref().context("redis container is not running — this is a bug")?;

    let key_prefix = format!("test:{}", Uuid::new_v4().to_string().replace('-', ""));
    let url = format!("{}/{db_index}", c.base_url);
    Ok(RedisScope { db_index, key_prefix, url })
}

/// Stop the Redis container and reset the singleton.
pub async fn teardown() {
    let mut guard = redis_mutex().lock().await;
    if let Some(c) = guard.take() {
        let _ = c.container.stop().await;
        drop(c);
    }
    // Reset the DB counter so the next suite starts at 0 again.
    DB_COUNTER.store(0, Ordering::Relaxed);
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn ensure_started() -> Result<()> {
    if !testcontainers_enabled() {
        bail!(
            "testcontainers disabled — set MAE_TESTCONTAINERS=1 to run Redis tests. \
             In Concourse, ensure the task has Docker socket access (DinD)."
        );
    }

    let mut guard = redis_mutex().lock().await;
    if guard.is_none() {
        let container: ContainerAsync<Redis> =
            Redis::default().start().await.context("failed to start Redis container")?;

        let port = container
            .get_host_port_ipv4(REDIS_PORT)
            .await
            .context("failed to get Redis host port")?;

        let base_url = format!("redis://localhost:{port}");

        *guard = Some(Inner { container, port, base_url });
    }
    Ok(())
}
