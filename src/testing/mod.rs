//! Testing utilities for Mae-based services.
//!
//! Enable with the `test-utils` Cargo feature:
//!
//! ```toml
//! [dev-dependencies]
//! mae = { ..., features = ["test-utils"] }
//! ```
//!
//! # Modules
//! | Module | Purpose |
//! |---|---|
//! | [`container`] | Docker container singletons (Postgres ✅, Redis/Neo4j/RabbitMQ ✅ #40) |
//! | [`context`]   | [`TestContext<C>`](context::TestContext) — generic test request context |
//! | [`env`]       | `.env` loader for test credentials |
//! | [`must`]      | Assertion helpers (`MustExpect`, `must_eq`, …) |

#[cfg(feature = "test-utils")]
pub mod container;

#[cfg(feature = "test-utils")]
pub mod context;

#[cfg(feature = "test-utils")]
pub mod env;

#[cfg(feature = "test-utils")]
pub mod must;
