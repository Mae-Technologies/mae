//! HTTP client for proxying requests between Mae micro-services.
//!
//! Downstream services are expected to respond with the standard [`ServiceResult`] envelope
//! (`{ "data": ... }` on success, `{ "error": ... }` on failure). The client unwraps the
//! `data` field and maps HTTP status codes back into [`Success`] / [`ServiceError`].

use crate::route::response::{ServiceError, ServiceResult, Success};
use reqwest::{Client, header};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Connection parameters for an inter-service HTTP client.
#[derive(Clone, Debug)]
pub struct ServiceClientConfig {
    pub base_url: String,
    pub user_id: i32,
    pub micro_service_key: String,
    pub micro_service_pass: String
}

/// Generic HTTP client that forwards the active session user and micro-service credentials.
pub struct HttpServiceClient {
    client: Client,
    base_url: String,
    user_id: i32,
    micro_service_key: String,
    micro_service_pass: String
}

fn unwrap_service_data(mut value: Value) -> Value {
    loop {
        match value {
            Value::Object(mut map) => {
                if map.len() == 1
                    && let Some(inner) = map.remove("data")
                {
                    value = inner;
                    continue;
                }
                return Value::Object(map);
            }
            _ => return value
        }
    }
}

fn extract_service_error_message(value: &Value) -> String {
    if let Some(message) = value.get("error").and_then(|v| v.as_str()) {
        return message.to_string();
    }
    if let Some(message) = value.as_str() {
        return message.to_string();
    }
    value.to_string()
}

fn map_http_status_to_error(status: reqwest::StatusCode, value: Value) -> ServiceError {
    let message = extract_service_error_message(&value);
    match status.as_u16() {
        400 => ServiceError::BadRequest(message),
        401 => ServiceError::Unauthorized,
        404 => ServiceError::BadGateway(format!("downstream not found: {message}")),
        409 => ServiceError::Conflict(message),
        422 => ServiceError::UnprocessableEntity(message),
        502 | 503 => ServiceError::BadGateway(format!("downstream unavailable: {message}")),
        _ => ServiceError::Unexpected(anyhow::anyhow!("HTTP {} - {}", status.as_u16(), message))
    }
}

fn map_http_status_to_success<R>(status: reqwest::StatusCode, data: R) -> ServiceResult<R>
where
    R: Serialize
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
    T: DeserializeOwned
{
    serde_json::from_value(unwrap_service_data(value))
        .map_err(|e| ServiceError::Unexpected(anyhow::anyhow!("deserialize failed: {e}")))
}

fn parse_response_body(status: reqwest::StatusCode, body: &str) -> Result<Value, ServiceError> {
    if body.trim().is_empty() {
        if status.is_success() {
            return Err(ServiceError::BadGateway("downstream returned empty success body".into()));
        }
        return Ok(Value::Null);
    }

    match serde_json::from_str::<Value>(body) {
        Ok(value) => Ok(value),
        Err(error) if status.is_success() => {
            Err(ServiceError::BadGateway(format!("downstream response parse failed: {error}")))
        }
        Err(_) => Ok(Value::Null)
    }
}

impl HttpServiceClient {
    pub fn new(config: ServiceClientConfig) -> Self {
        Self {
            client: Client::new(),
            base_url: config.base_url,
            user_id: config.user_id,
            micro_service_key: config.micro_service_key,
            micro_service_pass: config.micro_service_pass
        }
    }

    fn base_headers(&self) -> Result<header::HeaderMap, ServiceError> {
        let mut map = header::HeaderMap::new();

        let user_val = self.user_id.to_string().parse::<header::HeaderValue>().map_err(|e| {
            ServiceError::Unexpected(anyhow::anyhow!("invalid X-Session-User header: {e}"))
        })?;
        map.insert("X-Session-User", user_val);

        let name =
            header::HeaderName::from_bytes(self.micro_service_key.as_bytes()).map_err(|e| {
                ServiceError::Unexpected(anyhow::anyhow!("invalid service key header name: {e}"))
            })?;
        let val = self.micro_service_pass.parse::<header::HeaderValue>().map_err(|e| {
            ServiceError::Unexpected(anyhow::anyhow!("invalid service pass header value: {e}"))
        })?;
        map.insert(name, val);

        Ok(map)
    }

    fn json_headers(&self) -> Result<header::HeaderMap, ServiceError> {
        let mut map = self.base_headers()?;
        map.insert(header::CONTENT_TYPE, header::HeaderValue::from_static("application/json"));
        Ok(map)
    }

    async fn handle_response<R>(&self, response: reqwest::Response) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize
    {
        let status = response.status();
        let body = response.text().await.map_err(|error| {
            ServiceError::Unexpected(anyhow::anyhow!("read body failed: {error}"))
        })?;
        let value = parse_response_body(status, &body)?;

        if status.is_success() {
            map_http_status_to_success(status, deserialize_response(value)?)
        } else {
            Err(map_http_status_to_error(status, value))
        }
    }

    pub async fn get<R>(&self, path: &str) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize
    {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .headers(self.base_headers()?)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        self.handle_response(response).await
    }

    pub async fn post<R>(&self, path: &str, body: &Value) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize
    {
        let response = self
            .client
            .post(format!("{}{}", self.base_url, path))
            .headers(self.json_headers()?)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        self.handle_response(response).await
    }

    pub async fn put<R>(&self, path: &str, body: &Value) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize
    {
        let response = self
            .client
            .put(format!("{}{}", self.base_url, path))
            .headers(self.json_headers()?)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        self.handle_response(response).await
    }

    pub async fn delete<R>(&self, path: &str) -> ServiceResult<R>
    where
        R: DeserializeOwned + Serialize
    {
        let response = self
            .client
            .delete(format!("{}{}", self.base_url, path))
            .headers(self.base_headers()?)
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
        R: DeserializeOwned + Serialize
    {
        let method = reqwest::Method::from_bytes(b"QUERY")
            .map_err(|e| ServiceError::Unexpected(anyhow::anyhow!("invalid QUERY method: {e}")))?;

        let response = self
            .client
            .request(method, format!("{}{}", self.base_url, path))
            .headers(self.json_headers()?)
            .json(body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!(e))?;

        self.handle_response(response).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::response::ServiceError;
    use crate::testing::must::{Must, must_eq};
    use reqwest::StatusCode as ReqStatus;

    #[test]
    fn unwrap_service_data_strips_nested_data_envelope() {
        let input = serde_json::json!({"data": {"data": {"id": 1}}});
        let out = unwrap_service_data(input);
        must_eq(out["id"].as_i64().must(), 1);
    }

    #[test]
    fn extract_service_error_message_reads_error_field() {
        let msg = extract_service_error_message(&serde_json::json!({"error": "bad"}));
        must_eq(msg.as_str(), "bad");
    }

    #[test]
    fn map_http_status_to_error_maps_known_codes() {
        let err =
            map_http_status_to_error(ReqStatus::BAD_REQUEST, serde_json::json!({"error": "x"}));
        assert!(matches!(err, ServiceError::BadRequest(_)));

        let err = map_http_status_to_error(ReqStatus::UNAUTHORIZED, serde_json::json!({}));
        assert!(matches!(err, ServiceError::Unauthorized));

        let err = map_http_status_to_error(ReqStatus::NOT_FOUND, serde_json::json!({"error": "x"}));
        assert!(matches!(err, ServiceError::BadGateway(_)));

        let err = map_http_status_to_error(
            ReqStatus::UNPROCESSABLE_ENTITY,
            serde_json::json!({"error": "trial"})
        );
        assert!(matches!(err, ServiceError::UnprocessableEntity(_)));
    }

    #[test]
    fn parse_response_body_maps_empty_error_body_to_null() {
        let value = parse_response_body(ReqStatus::NOT_FOUND, "").expect("null body");
        assert!(value.is_null());

        let err = parse_response_body(ReqStatus::OK, "").expect_err("empty success body");
        assert!(matches!(err, ServiceError::BadGateway(_)));
    }

    #[test]
    fn parse_response_body_maps_empty_404_to_bad_gateway() {
        let value = parse_response_body(ReqStatus::NOT_FOUND, "").expect("null body");
        let err = map_http_status_to_error(ReqStatus::NOT_FOUND, value);
        assert!(matches!(err, ServiceError::BadGateway(_)));
    }

    #[test]
    fn parse_response_body_maps_invalid_error_body_to_null() {
        let value = parse_response_body(ReqStatus::NOT_FOUND, "not json").expect("null body");
        assert!(value.is_null());
    }

    #[test]
    fn map_http_status_to_success_maps_200_and_201() {
        assert!(map_http_status_to_success(ReqStatus::OK, serde_json::json!({"a": 1})).is_ok());
        assert!(
            map_http_status_to_success(ReqStatus::CREATED, serde_json::json!({"a": 1})).is_ok()
        );
        assert!(
            map_http_status_to_success(ReqStatus::ACCEPTED, serde_json::json!({"a": 1})).is_ok()
        );
    }

    #[test]
    fn deserialize_response_unwraps_data_field() {
        let value = serde_json::json!({"data": {"name": "widget"}});
        let parsed: serde_json::Value = deserialize_response(value).expect("deserialize");
        must_eq(parsed["name"].as_str().must(), "widget");
    }

    #[test]
    fn client_builds_auth_headers() {
        let client = HttpServiceClient::new(ServiceClientConfig {
            base_url: "http://localhost:8080".to_string(),
            user_id: 7,
            micro_service_key: "X-Service-Key".to_string(),
            micro_service_pass: "secret-pass".to_string()
        });

        let headers = client.base_headers().expect("headers");
        must_eq(headers.get("X-Session-User").expect("user").to_str().expect("str"), "7");
        must_eq(headers.get("X-Service-Key").expect("key").to_str().expect("str"), "secret-pass");

        let json_headers = client.json_headers().expect("json headers");
        must_eq(
            json_headers.get(reqwest::header::CONTENT_TYPE).expect("ct").to_str().expect("str"),
            "application/json"
        );
    }
}
