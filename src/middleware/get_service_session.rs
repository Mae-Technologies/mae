//! Service-to-service session middleware.
//!
//! Microservices that communicate directly with each other (e.g. via internal HTTP
//! routes not exposed to end users) need their own session-validation path so that
//! the standard user-facing middleware can remain untouched and independently auditable.
//!
//! ## Why separate from `get_session`?
//!
//! `get_session` is the standard middleware attached to user-facing routes. It validates
//! cookies / bearer tokens issued to human users. `get_service_session` is the entry
//! point for internal service calls where the caller is another Mae microservice rather
//! than a browser or mobile client. Keeping them separate means:
//!
//! - **Explicit call sites** — it is immediately obvious from the route wiring which
//!   endpoints are internal-only vs user-facing.
//! - **Future divergence** — service auth may evolve (e.g. mTLS, service accounts) without
//!   touching user-session logic.
//! - **Auditability** — security reviewers can grep for `get_service_session` to find all
//!   internal endpoints at once.
//!
//! ## When to use `get_service_session` vs `get_session`
//!
//! | Scenario | Middleware |
//! |---|---|
//! | Route called by a browser / mobile client | `get_session` |
//! | Route called by another Mae microservice | `get_service_session` |
//! | Public / unauthenticated route | neither |
//!
//! Currently the implementation delegates directly to `get_session` because service
//! auth is validated the same way as user auth. Once service-specific validation is
//! added (e.g. service-account token checks), change only this function.

use crate::middleware::get_session;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;

pub async fn get_service_session(
    req: ServiceRequest,
    next: Next<impl MessageBody,>,
) -> Result<ServiceResponse<impl MessageBody,>, actix_web::Error,> {
    // Service-to-service session handling currently mirrors standard session middleware.
    // Kept separate for explicit call sites in microservice run wiring.
    get_session(req, next,).await
}
