//! Container singletons for mae integration tests.
//!
//! Each sub-module owns one Docker container type and exposes:
//! - A static singleton (started lazily, guarded by `MAE_TESTCONTAINERS=1`).
//! - A per-test **isolation scope** (schema, keyspace, vhost, database) that
//!   prevents cross-test interference.
//! - A [`teardown`](MaeContainer::teardown) function that stops the container
//!   and clears the singleton so the next test-run starts fresh.
//!
//! All four container types implement [`MaeContainer`].  Call
//! [`teardown_all()`] from your global test teardown hook to stop every
//! container in one shot.
//!
//! # Stability contract for issue #40
//! The trait and module layout are intentionally fixed here.  Issue #40 should
//! fill in the `start()` implementations for Redis, Neo4j, and RabbitMQ while
//! keeping the public API below unchanged.

pub mod neo4j;
pub mod postgres;
pub mod rabbitmq;
pub mod redis;

use std::future::Future;

/// Common interface shared by every Mae container singleton.
///
/// Implementors are zero-sized unit types (`struct PostgresContainer;` etc.)
/// that act as namespaces — all state lives in module-level statics.
pub trait MaeContainer {
    /// Isolation scope created per test (schema, keyspace, vhost, …).
    type Scope: Send + 'static;

    /// Start (or return the already-running) container singleton.
    ///
    /// Returns `Some(())` when the container is running, `None` when
    /// `MAE_TESTCONTAINERS` is not set to `1`/`true`.
    fn start() -> impl Future<Output = Option<(),>,> + Send;

    /// Create a fresh isolation scope for one test.
    ///
    /// Callers must drop / clean up the scope when the test finishes.
    fn scope() -> impl Future<Output = anyhow::Result<Self::Scope,>,> + Send;

    /// Stop the container and reset the singleton.
    ///
    /// Safe to call even when the container was never started.
    fn teardown() -> impl Future<Output = (),> + Send;
}

/// Stop **all** Mae container singletons.
///
/// Call this from your test harness's global teardown hook (e.g. a `#[dtor]`
/// or an `atexit`-style function registered in `main`).
///
/// Containers are torn down sequentially to avoid races on shared resources.
pub async fn teardown_all() {
    postgres::PostgresContainer::teardown().await;
    redis::RedisContainer::teardown().await;
    neo4j::Neo4jContainer::teardown().await;
    rabbitmq::RabbitMqContainer::teardown().await;
}
