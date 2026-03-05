//! Testcontainer singletons for integration tests.
//!
//! Each container is started lazily on first access and shared for the
//! lifetime of the test process, or until [`teardown`] is called.
//!
//! ## Environment gate
//!
//! All containers are gated behind `MAE_TESTCONTAINERS=1`.  When the
//! variable is absent or set to any other value, getter functions return
//! `Err` with a human-readable message so tests can be skipped gracefully
//! in environments without Docker.
//!
//! ## Per-test isolation
//!
//! Each module exposes an isolation helper that returns a unique handle
//! for the test (Redis DB number, Neo4j label prefix, RabbitMQ vhost).
//! Using these keeps tests independent without requiring separate
//! containers per test.
//!
//! ## Docker-in-Docker (Concourse CI)
//!
//! Testcontainers-rs respects the `DOCKER_HOST` and
//! `TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE` environment variables.
//! Concourse workers with DinD expose the Docker socket at
//! `/var/run/docker.sock` — the default — so no extra configuration
//! is needed.  If your Concourse task uses a non-standard socket path,
//! set `DOCKER_HOST=unix:///path/to/docker.sock` in the task environment.
//!
//! ## Teardown
//!
//! Every module exposes an `async fn teardown()` that stops the container
//! and resets the singleton so the next call to a getter will restart a
//! fresh container.

pub mod neo4j;
pub mod rabbitmq;
pub mod redis;

/// Stops all running testcontainers managed by this module.
///
/// Calls teardown on each sub-module in parallel and collects errors.
pub async fn teardown_all() {
    neo4j::teardown().await;
    redis::teardown().await;
    rabbitmq::teardown().await;
}
