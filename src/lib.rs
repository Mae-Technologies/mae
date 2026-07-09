//! Opinionated async Rust framework for building Mae-Technologies micro-services.
//!
//! Use with [`mae_macros`](https://crates.io/crates/mae_macros) (`#[run_app]`, `#[schema]`,
//! `#[mae_test]`). API request/response types belong in your service or a models crate.
//!
//! # Public surface (stable for service authors)
//!
//! - [`app`] — configuration, `DeriveContext`, Actix server `run`
//! - [`context`] — [`RequestContext`] (pool + session + custom config per request)
//! - [`repo`] — typed Postgres layer: [`WithExecutor`](repo::WithExecutor), filters, [`DomainStatus`](repo::default::DomainStatus)
//! - [`posting`] — multi-step post orchestration ([`PostingController`](posting::PostingController))
//! - [`route`] — [`Success`](route::response::Success), [`ServiceError`](route::response::ServiceError), health routes
//! - [`middleware`] — session / micro-service auth (installed by `#[run_app]`)
//! - [`session`] — logged-in user identity
//! - [`service`] — [`HttpServiceClient`](service::HttpServiceClient) for downstream HTTP
//! - [`crypto`] / [`totp`] — optional field encryption and TOTP helpers
//! - [`util`] — small shared helpers
//! - [`testing`] — integration-test utilities (`test-utils` feature only)
//!
//! Internal modules (`repo::__private__`, container refcount guards, etc.) are not API.

#![deny(clippy::disallowed_methods)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod app;
pub mod context;
pub mod crypto;
pub mod middleware;
pub mod posting;
pub mod repo;
pub mod route;
pub mod service;
pub mod session;
pub mod telemetry;
pub mod testing;
pub mod totp;
pub mod util;
