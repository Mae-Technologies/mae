//! Built-in health check handlers for mae applications.
//!
//! These are automatically registered by the `#[run_app]` macro — no
//! service-level code needed.

use actix_web::{get, web, HttpResponse};
use sqlx::PgPool;

/// Basic liveness probe — always returns 200.
#[get("/health")]
pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

/// Postgres readiness probe — checks DB connectivity.
#[get("/health-pg")]
pub async fn health_pg(pool: web::Data<PgPool>) -> HttpResponse {
    match sqlx::query("SELECT 1").execute(pool.get_ref()).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ok", "db": "postgres"})),
        Err(_) => HttpResponse::ServiceUnavailable()
            .json(serde_json::json!({"status": "error", "db": "postgres"})),
    }
}

/// Neo4j readiness probe — checks graph DB connectivity.
#[get("/health-neo")]
pub async fn health_neo(graph: web::Data<neo4rs::Graph>) -> HttpResponse {
    match graph.run(neo4rs::query("RETURN 1 AS _n")).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ok", "db": "neo4j"})),
        Err(_) => HttpResponse::ServiceUnavailable()
            .json(serde_json::json!({"status": "error", "db": "neo4j"})),
    }
}
