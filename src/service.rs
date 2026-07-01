//! HTTP client for proxying requests between Mae micro-services.
//!
//! Downstream services are expected to respond with the standard [`ServiceResult`] envelope
//! (`{ "data": ... }` on success, `{ "error": ... }` on failure). The client unwraps the
//! `data` field and maps HTTP status codes back into [`Success`] / [`ServiceError`].

use crate::route::response::{ServiceError, ServiceResult, Success};
use anyhow::Context;
use reqwest::{header, Client};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Connection parameters for an inter-service HTTP client.
#[derive(Clone, Debug)]
pub struct ServiceClientConfig {
    pub base_url: String,
    pub user_id: i32,
    pub micro_service_key: String,
    pub micro_service_pass: String,
}

/// Generic HTTP client that forwards the active session user and micro-service credentials.
pub struct HttpServiceClient {
    client: Client,
    base_url: String,
    user_id: i32,
    micro_service_key: String,
    micro_service_pass: String,
}

fn unwrap_service_data(mut value: Value) -> Value {
    loop {
        match value {
            Value::Object(mut map) => {
                if map.len() == 1 {
                    if let Some(inner) = map.remove("data") {
                        value = inner;
                        continue;
                    }
                }
                return Value::Object(map);
            }
            _ => return value,
        }
    }
}

fn map_http_status_to_error(status: reqwest::StatusCode, value: Value) -> ServiceError {
    match status.as_u16() {
        400 => ServiceError::BadRequest(value.to_string()),
        401 => ServiceError::Unauthorized,
        404 => ServiceError::NotFound(value.to_string()),
        409 => ServiceError::Conflict(value.to_string()),
        _ => ServiceError::Unexpected(anyhow::anyhow!(
            "HTTP {} - {}",
            status.as_u16(),
            value
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
        )),
    }
}

fn map_http_status_to_success<R>(status: reqwest::StatusCode, data: R) -> ServiceResult<R>
where
    R: Serialize,
{
    match status.as_u16() {
        200 => Success::ok(data),
        201 => Success::created(data),
        code => {
            if let Ok(s) = actix_web::http::StatusCode::from_u16(code) {
                Success::with_status(data, s)
            } else {
                Success::ok(data)
            }
        }
    }
}

fn deserialize_response<T>(value: Value) -> Result<T, ServiceError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(unwrap_service_data(value))
        .map_err(|e| ServiceError::Unexpected(anyhow::anyhow!("deserialize failed: {e}")))
}

impl HttpServiceClient {
    pub fn new(config: ServiceClientConfig) -> Self {
        Self {
            client: Client::new(),
            base_url: config.base_url,
            user_id: config.user_id,
            micro_service_key: config.micro_service_key,
            micro_service_pass: config.micro_service_pass,
        }
    }

    fn headers(&self) -> Result<header::HeaderMap, ServiceError> {
        let mut map = header::HeaderMap::new();

        let user_val = self
            .user_id
            .to_string()
            .parse::<header::HeaderValue>()
            .map_err(|e| {
                ServiceError::Unexpected(anyhow::anyhow!("invalid X-Session-User header: {e}"))
            })?;
        map.insert("X-Session-User", user_val);

        let name = header::HeaderName::from_bytes(self.micro_service_key.as_bytes()).map_err(
            |e| ServiceError::Unexpected(anyhow::anyhow!("invalid service key header name: {e}")),
        )?;
        let val = self.micro_service_pass.parse::<header::HeaderValue>().map_err(|e| {
            ServiceError::Unexpected(anyhow::anyhow!("invalid service pass header value: {e}"))
        })?;
        map.insert(name, val);

        map.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        Ok(map)
    }

    async fn handle_response<R>(&self, response: reqwest::Response) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize,
    {
        let status = response.status();
        let value = response.json::<Value>().await.context("parse failed")?;

        if status.is_success() {
            map_http_status_to_success(status, deserialize_response(value)?)
        } else {
            Err(map_http_status_to_error(status, value))
        }
    }

    pub async fn get<R>(&self, path: &str) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize,
    {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .headers(self.headers()?)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        self.handle_response(response).await
    }

    pub async fn post<R>(&self, path: &str, body: &Value) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize,
    {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .headers(self.headers()?)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        self.handle_response(response).await
    }

    pub async fn put<R>(&self, path: &str, body: &Value) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize,
    {
        let response = self
            .client
            .put(format!("{}{}", self.base_url, path))
            .headers(self.headers()?)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        self.handle_response(response).await
    }

    /// Send a `QUERY` request with a JSON body.
    ///
    /// Mae services use the custom `QUERY` HTTP method for list endpoints that accept a typed
    /// [`crate::route::ListQuery`] payload instead of URL query parameters.
    pub async fn query<R>(&self, path: &str, body: &Value) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize,
    {
        let method = reqwest::Method::from_bytes(b"QUERY").map_err(|e| {
            ServiceError::Unexpected(anyhow::anyhow!("invalid QUERY method: {e}"))
        })?;

        let response = self
            .client
            .request(method, format!("{}{}", self.base_url, path))
            .headers(self.headers()?)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        self.handle_response(response).await
    }
}