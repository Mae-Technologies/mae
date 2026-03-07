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
/// let err = e401("not authenticated");
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
