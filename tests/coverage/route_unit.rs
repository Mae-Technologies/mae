use actix_web::test as actix_test;
use actix_web::{App, Responder, ResponseError};
use mae::route::response::{ServiceError, Success};
use mae::testing::must::must_eq;

#[actix_web::test]
async fn success_responder_returns_json_body() {
    let success = Success::ok(serde_json::json!({"ok": true})).expect("success");
    let req = actix_test::TestRequest::default().to_http_request();
    let resp = success.respond_to(&req);
    must_eq(resp.status(), actix_web::http::StatusCode::OK);
}

#[test]
fn service_error_helper_constructors_exist() {
    let e500 = mae::route::response::e500("boom");
    must_eq(
        e500.as_response_error().status_code(),
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    );

    let e401 = mae::route::response::e401("nope");
    must_eq(e401.as_response_error().status_code(), actix_web::http::StatusCode::UNAUTHORIZED);

    let e400 = mae::route::response::e400("bad");
    must_eq(e400.as_response_error().status_code(), actix_web::http::StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn health_route_still_wired() {
    use mae::route::health::health;

    let app = actix_test::init_service(App::new().service(health)).await;
    let req = actix_test::TestRequest::get().uri("/health").to_request();
    let resp = actix_test::call_service(&app, req).await;
    must_eq(resp.status().as_u16(), 200);
}

#[test]
fn unexpected_error_maps_to_internal_server_error() {
    let err = ServiceError::Unexpected(anyhow::anyhow!("db down"));
    must_eq(err.error_response().status(), actix_web::http::StatusCode::INTERNAL_SERVER_ERROR);
}
