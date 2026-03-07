//! Opinionated async Rust framework for building Mae-Technologies micro-services.
//!
//! # Modules
//!
//! - [`app`] — application lifecycle: configuration, builder, HTTP server runner
//! - [`repo`] — typed async repository layer over SQLx/Postgres
//! - [`middleware`] — Actix-Web extractors for sessions and service auth
//! - [`telemetry`] — structured tracing/logging setup
//! - [`session`] — session identity type
//! - [`error_response`] — standardised HTTP error responses
//! - [`testing`] — test utilities (enabled via `test-utils` feature)

#![deny(clippy::disallowed_methods)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod app;
pub mod error_response;
pub mod middleware;
pub mod repo;
pub mod request_context;
pub mod routes;
pub mod session;
pub mod telemetry;
pub mod testing;
pub mod util;
