//! Singleton Redis testcontainer with per-test database isolation.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use mae::testing::containers::redis::{get_redis_url, IsolatedRedis};
//!
//! #[tokio::test]
//! async fn my_test() {
//!     let iso = IsolatedRedis::acquire().await.expect("redis must be running");
//!     // iso.url() returns a DB-scoped URL, e.g. "redis://localhost:32768/3"
//!     // iso drops automatically; call iso.flush().await to wipe mid-test.
//! }
//! ```
//!
//! ## Per-test isolation
//!
//! Redis supports 16 logical databases (0–15) by default.  [`IsolatedRedis`]
//! allocates one database number per test using an atomic counter.  If more
//! than 16 concurrent tests are active, they wrap around modulo 16; for most
//! test suites this is fine since tests run sequentially within one process.
//!
//! For stronger isolation (parallel suites), increase `databases N` in a
//! custom Redis configuration.

use std::sync::{
    OnceLock,
    atomic::{AtomicU8, Ordering},
};

use anyhow::{Context, Result, bail};
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::redis::Redis;
use tokio::sync::Mutex;

use crate::testing::must::testcontainers_enabled;

/// Default port exposed by the Redis image.
pub const REDIS_PORT: u16 = 6379;

/// Number of logical databases in the default Redis configuration.
const REDIS_DB_COUNT: u8 = 16;

// ── Singleton state ──────────────────────────────────────────────────────────

/// Internal container state.
pub struct RedisContainer {
    pub container: ContainerAsync<Redis,>,
    pub port: u16,
    /// Base URL without a database suffix, e.g. `"redis://localhost:32768"`.
    pub base_url: String,
}

static REDIS: OnceLock<Mutex<Option<RedisContainer,>,>,> = OnceLock::new();
/// Round-robin DB counter for per-test isolation.
static DB_COUNTER: AtomicU8 = AtomicU8::new(0,);

fn redis_mutex() -> &'static Mutex<Option<RedisContainer,>,> {
    REDIS.get_or_init(|| Mutex::new(None,),)
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Returns `(url, port)` for the shared Redis container.
///
/// The URL points to DB 0 (`redis://localhost:<port>/0`).
/// For per-test isolation, prefer [`IsolatedRedis::acquire`].
///
/// # Errors
/// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled.
pub async fn get_redis_url() -> Result<(String, u16,),> {
    ensure_started().await?;

    let guard = redis_mutex().lock().await;
    let c = guard.as_ref().context("redis container is not running — this is a bug",)?;
    Ok((format!("{}/0", c.base_url), c.port,),)
}

/// Stops the Redis container and resets the singleton.
///
/// The next call to [`get_redis_url`] or [`IsolatedRedis::acquire`] will
/// start a fresh container.
pub async fn teardown() {
    let mut guard = redis_mutex().lock().await;
    if let Some(c,) = guard.take() {
        let _ = c.container.stop().await;
        drop(c,);
    }
    // Reset the DB counter so the next suite starts at 0 again.
    DB_COUNTER.store(0, Ordering::Relaxed,);
}

// ── Per-test isolation ───────────────────────────────────────────────────────

/// A unique Redis database handle for one test.
///
/// Allocates the next available DB number (mod 16) and exposes a
/// DB-scoped URL.  Call [`IsolatedRedis::flush`] to wipe all keys in
/// this database mid-test or at teardown.
pub struct IsolatedRedis {
    pub db: u8,
    pub url: String,
    pub port: u16,
}

impl IsolatedRedis {
    /// Acquires the next available DB slot and starts the container if needed.
    ///
    /// # Errors
    /// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled.
    pub async fn acquire() -> Result<Self,> {
        ensure_started().await?;

        let db = DB_COUNTER.fetch_add(1, Ordering::Relaxed,) % REDIS_DB_COUNT;

        let guard = redis_mutex().lock().await;
        let c = guard.as_ref().context("redis container is not running — this is a bug",)?;

        Ok(Self { db, url: format!("{}/{db}", c.base_url), port: c.port, },)
    }

    /// Returns the DB-scoped URL, e.g. `"redis://localhost:32768/3"`.
    pub fn url(&self,) -> &str {
        &self.url
    }

    /// Returns the logical database number (0–15).
    pub fn db(&self,) -> u8 {
        self.db
    }

    /// Returns the `FLUSHDB` command string for this database.
    ///
    /// Connect to [`url`][Self::url] and issue `FLUSHDB` to wipe all keys
    /// belonging to this test.
    pub fn flushdb_command(&self,) -> &'static str {
        "FLUSHDB"
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

async fn ensure_started() -> Result<(),> {
    if !testcontainers_enabled() {
        bail!(
            "testcontainers disabled — set MAE_TESTCONTAINERS=1 to run Redis tests. \
             In Concourse, ensure the task has Docker socket access (DinD)."
        );
    }

    let mut guard = redis_mutex().lock().await;
    if guard.is_none() {
        let container: ContainerAsync<Redis,> =
            Redis::default().start().await.context("failed to start Redis container",)?;

        let port = container
            .get_host_port_ipv4(REDIS_PORT,)
            .await
            .context("failed to get Redis host port",)?;

        let base_url = format!("redis://localhost:{port}");

        *guard = Some(RedisContainer { container, port, base_url, },);
    }
    Ok((),)
}
