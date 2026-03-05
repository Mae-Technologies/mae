mod get_context;
mod get_service_session;
mod get_session;
pub use get_context::*;
pub use get_session::*;

/// Re-exports [`get_service_session`] for use in microservice route wiring.
///
/// Consumed via `mae::app::prelude::get_service_session` by any service that needs
/// to register internal (service-to-service) routes. See the module-level docs in
/// `get_service_session.rs` for the full rationale.
pub use get_service_session::*;
