//! Header-based authentication for internal microservice HTTP calls.
//!
//! Validates the caller using a configurable header name/value pair (sent by
//! [`HttpServiceClient`](crate::service::HttpServiceClient)) and injects
//! [`Session`](crate::session::Session) from the `X-Session-User` header.

use crate::session::Session;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::middleware::Next;
use actix_web::{web, HttpMessage, HttpResponse};

/// Service context fields required for header-key microservice authentication.
pub trait MicroserviceAuth {
    fn micro_service_key(&self) -> &str;
    fn micro_service_pass(&self) -> &str;
}

pub async fn get_microservice_session<T>(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error>
where
    T: MicroserviceAuth + Clone + 'static,
{
    let app_ctx = req.app_data::<web::Data<T>>().ok_or_else(|| {
        InternalError::from_response(
            "Missing app context",
            HttpResponse::InternalServerError().finish(),
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

    if pass_ok {
        req.extensions_mut().insert(Session(user_id));
        return next.call(req).await;
    }

    let resp = HttpResponse::Unauthorized().finish();
    Err(InternalError::from_response("Unauthorized.", resp).into())
}