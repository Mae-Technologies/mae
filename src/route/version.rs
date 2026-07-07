//! Build metadata endpoint for deployment compatibility checks.

use actix_web::{HttpResponse, get};
use serde::Serialize;

#[derive(Serialize)]
struct VersionResponse {
    service: String,
    git_sha: String,
    mae_version: String,
    image_tag: String
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Returns build metadata injected at image build time (`MAE_GIT_SHA`, `MAE_IMAGE_TAG`).
///
/// Registered automatically by the `#[run_app]` macro on `GET /version`.
#[get("/version")]
pub async fn version() -> HttpResponse {
    HttpResponse::Ok().json(VersionResponse {
        service: env_or("MAE_SERVICE_NAME", "unknown"),
        git_sha: env_or("MAE_GIT_SHA", "unknown"),
        mae_version: env_or("CARGO_PKG_VERSION", env!("CARGO_PKG_VERSION")),
        image_tag: env_or("MAE_IMAGE_TAG", "unknown")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, test};

    #[actix_web::test]
    async fn version_returns_200() {
        let app = test::init_service(App::new().service(version)).await;
        let req = test::TestRequest::get().uri("/version").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
    }
}
