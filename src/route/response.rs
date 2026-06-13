use crate::util;
use actix_web::{HttpRequest, HttpResponse, Responder, body::BoxBody, http::StatusCode};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Serialize)]
pub struct Success<T>
where
    T: Serialize
{
    pub data: T,

    #[serde(skip)]
    status: StatusCode
}

impl<T> Success<T>
where
    T: Serialize
{
    pub fn ok(data: T) -> Result<Self, ServiceError> {
        Ok(Self { data, status: StatusCode::OK })
    }

    pub fn created(data: T) -> Result<Self, ServiceError> {
        Ok(Self { data, status: StatusCode::CREATED })
    }

    pub fn with_status(data: T, status: StatusCode) -> Result<Self, ServiceError> {
        Ok(Self { data, status })
    }
}

impl<T> Responder for Success<T>
where
    T: Serialize
{
    type Body = BoxBody;

    fn respond_to(self, _req: &HttpRequest) -> HttpResponse<Self::Body> {
        HttpResponse::build(self.status).json(self)
    }
}

pub type ServiceResult<T> = Result<Success<T>, ServiceError>;

#[derive(Error)]
pub enum ServiceError {
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),

    #[error("unauthorized")]
    Unauthorized,

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String)
}

impl std::fmt::Debug for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        util::error_chain_fmt(self, f)
    }
}

impl actix_web::ResponseError for ServiceError {
    fn status_code(&self) -> StatusCode {
        match self {
            ServiceError::Unexpected(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServiceError::Unauthorized => StatusCode::UNAUTHORIZED,
            ServiceError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServiceError::NotFound(_) => StatusCode::NOT_FOUND,
            ServiceError::Conflict(_) => StatusCode::CONFLICT
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();

        HttpResponse::build(status).json(serde_json::json!({
            "error": self.to_string()
        }))
    }
}
/// Convert any displayable error into an HTTP 500 Internal Server Error response.
///
/// # Examples
///
/// ```
/// use mae::error_response::e500;
///
/// let err = e500("something went wrong");
/// assert_eq!(err.as_response_error().status_code(), actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
/// ```
pub fn e500<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static
{
    actix_web::error::ErrorInternalServerError(e)
}

/// Convert any displayable error into an HTTP 401 Unauthorized response.
///
/// # Examples
///
/// ```
/// use mae::error_response::e401;
///
/// let err = e401("unauthorized");
/// assert_eq!(err.as_response_error().status_code(), actix_web::http::StatusCode::UNAUTHORIZED);
/// ```
pub fn e401<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static
{
    actix_web::error::ErrorUnauthorized(e)
}

/// Convert any displayable error into an HTTP 400 Bad Request response.
///
/// # Examples
///
/// ```
/// use mae::error_response::e400;
///
/// let err = e400("invalid input");
/// assert_eq!(err.as_response_error().status_code(), actix_web::http::StatusCode::BAD_REQUEST);
/// ```
pub fn e400<T>(e: T) -> actix_web::Error
where
    T: std::fmt::Debug + std::fmt::Display + 'static
{
    actix_web::error::ErrorBadRequest(e)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::must::*;
    use actix_web::ResponseError;

    #[test]
    fn service_error_unauthorized_display() {
        let e = ServiceError::Unauthorized;
        must_eq(e.to_string().as_str(), "unauthorized");
    }

    #[test]
    fn service_error_bad_request_display() {
        let e = ServiceError::BadRequest("bad input".to_string());
        must_eq(e.to_string().as_str(), "bad input");
    }

    #[test]
    fn service_error_unexpected_display() {
        let e = ServiceError::Unexpected(anyhow::anyhow!("something went wrong"));
        must_be_true(e.to_string().contains("something went wrong"));
    }

    #[test]
    fn service_error_debug_format() {
        let e = ServiceError::Unauthorized;
        let debug_str = format!("{:?}", e);
        must_be_true(!debug_str.is_empty());
    }

    #[test]
    fn service_error_unauthorized_response_is_401() {
        let e = ServiceError::Unauthorized;
        let resp = e.error_response();
        must_eq(resp.status(), actix_web::http::StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn service_error_bad_request_response_is_400() {
        let e = ServiceError::BadRequest("bad".to_string());
        let resp = e.error_response();
        must_eq(resp.status(), actix_web::http::StatusCode::BAD_REQUEST);
    }

    #[test]
    fn service_error_unexpected_response_is_500() {
        let e = ServiceError::Unexpected(anyhow::anyhow!("internal error"));
        let resp = e.error_response();
        must_eq(resp.status(), actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn success_struct_serializes() {
        let s = Success::ok(serde_json::json!({"key": "val"})).unwrap();
        let json = serde_json::to_value(&s).must();
        must_eq(json["data"]["key"].as_str().must(), "val");
    }
}
