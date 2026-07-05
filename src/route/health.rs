//! Built-in health check handlers for mae applications.
//!
//! These three handlers are automatically registered by the `#[run_app]` macro
//! so every service gets liveness and readiness probes with zero service-level
//! code.
//!
//! # Endpoints
//!
//! | Route | Purpose | Success | Failure |
//! |---|---|---|---|
//! | `GET /health` | Liveness — always 200 | `{"status":"ok"}` | — |
//! | `GET /health-pg` | Postgres readiness | `{"status":"ok","db":"postgres"}` | 503 |
//! | `GET /health-neo` | Neo4j readiness | `{"status":"ok","db":"neo4j"}` | 503 |

use actix_web::{HttpResponse, get, web};
use sqlx::PgPool;

/// Liveness probe — always returns 200 `{"status":"ok"}`.
///
/// Registered automatically by the `#[run_app]` macro on `GET /health`.
#[get("/health")]
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

/// Postgres readiness probe.
///
/// Runs `SELECT 1` against the injected [`PgPool`].
/// Returns 200 on success, 503 on failure.
///
/// Registered automatically by the `#[run_app]` macro on `GET /health-pg`.
#[get("/health-pg")]
pub async fn health_pg(pool: web::Data<PgPool>) -> HttpResponse {
    match sqlx::query("SELECT 1").execute(pool.get_ref()).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ok", "db": "postgres"})),
        Err(_) => HttpResponse::ServiceUnavailable()
            .json(serde_json::json!({"status": "error", "db": "postgres"}))
    }
}

/// Neo4j readiness probe.
///
/// Runs `RETURN 1 AS _n` against the injected [`neo4rs::Graph`].
/// Returns 200 on success, 503 on failure.
///
/// Registered automatically by the `#[run_app]` macro on `GET /health-neo`.
#[get("/health-neo")]
pub async fn health_neo(graph: web::Data<neo4rs::Graph>) -> HttpResponse {
    match graph.run(neo4rs::query("RETURN 1 AS _n")).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ok", "db": "neo4j"})),
        Err(_) => HttpResponse::ServiceUnavailable()
            .json(serde_json::json!({"status": "error", "db": "neo4j"}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, test};

    #[actix_web::test]
    async fn health_always_returns_200() {
        let app = test::init_service(App::new().service(health)).await;
        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200u16);
    }
}
