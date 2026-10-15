//! Header-based authentication for internal microservice HTTP calls.
//!
//! Validates the caller using a configurable header name/value pair (sent by
//! [`HttpServiceClient`](crate::service::HttpServiceClient)) and injects
//! [`Session`](crate::session::Session) from the `X-Session-User` header.
//! When that user id is present, the grant snapshot is loaded from Redis
//! through [`MicroserviceAuth::grant_store`] and stored on the [`Session`]
//! (the second field). That is the only write onto the request. The store
//! itself cannot edit Redis. Allow-lists are not headers.

use crate::grants::{GrantDecision, GrantStore, Grants, GrantsError, resolve_grants};
use crate::session::Session;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::http::StatusCode;
use actix_web::middleware::Next;
use actix_web::{HttpMessage, HttpResponse, web};

/// Service context fields required for header-key microservice authentication.
pub trait MicroserviceAuth {
    fn micro_service_key(&self) -> &str;
    fn micro_service_pass(&self) -> &str;

    /// Redis snapshot store. The default leaves user-scoped calls unavailable
    /// until the service holds a [`GrantStore`] on its context.
    fn grant_store(&self) -> Option<&GrantStore> {
        None
    }
}

fn reject(status: StatusCode) -> actix_web::Error {
    InternalError::from_response("", HttpResponse::build(status).finish()).into()
}

pub async fn get_microservice_session<T>(
    req: ServiceRequest,
    next: Next<impl MessageBody>
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error>
where
    T: MicroserviceAuth + Clone + 'static
{
    let app_ctx = req.app_data::<web::Data<T>>().ok_or_else(|| {
        InternalError::from_response(
            "Missing app context",
            HttpResponse::InternalServerError().finish()
        )
    })?;

    let key_name = app_ctx.micro_service_key();
    let expected_pass = app_ctx.micro_service_pass();

    let pass_ok = req
        .headers()
        .get(key_name)
        .and_then(|h| h.to_str().ok())
        .map(|v| v == expected_pass)
        .unwrap_or(false);

    let user_id = req
        .headers()
        .get("X-Session-User")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i32>().ok());

    // Clone the store before any await so the request borrow ends.
    let store = req.app_data::<web::Data<T>>().and_then(|ctx| ctx.grant_store().cloned());

    if pass_ok {
        let grants = match user_id {
            None => Grants::open(),
            Some(id) => {
                let loaded = match &store {
                    Some(store) => store.get(id).await,
                    None => {
                        tracing::warn!(user_id = id, "grant store is not configured");
                        Err(GrantsError::Unavailable)
                    }
                };
                match resolve_grants(Some(id), loaded) {
                    GrantDecision::Apply(grants) => grants,
                    GrantDecision::Unavailable => {
                        return Err(reject(StatusCode::SERVICE_UNAVAILABLE));
                    }
                    GrantDecision::Open | GrantDecision::Deny => {
                        return Err(reject(StatusCode::UNAUTHORIZED));
                    }
                }
            }
        };
        req.extensions_mut().insert(Session(user_id, grants));
        return next.call(req).await;
    }

    Err(reject(StatusCode::UNAUTHORIZED))
}
