//! Actix-Web middleware for session validation and request context injection.
//!
//! This module provides three middleware functions for use with Actix-Web's
//! `from_fn` middleware API:
//!
//! - [`get_session`] — validates the Redis-backed session cookie and inserts
//!   the extracted [`Session`](crate::session::Session) data into request
//!   extensions. Use on all user-facing authenticated routes.
//! - [`get_service_session`] — identical behaviour to `get_session` but
//!   explicitly marks a route as internal (service-to-service). Kept separate
//!   for auditability and future divergence.
//! - [`get_context`] — extends `get_session` by also resolving a
//!   service-specific context type `T` from app data and attaching a full
//!   [`RequestContext`](crate::request_context::RequestContext) to the
//!   request. **Note:** currently does not work at runtime (see source).

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
