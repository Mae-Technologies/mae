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
pub mod util;
