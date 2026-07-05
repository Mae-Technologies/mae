use mae::telemetry::{get_subscriber, spawn_blocking_with_tracing};
use mae::testing::must::must_eq;

#[test]
fn get_subscriber_builds_without_panicking() {
    let _subscriber =
        get_subscriber("mae-coverage".to_string(), "info".to_string(), std::io::sink);
}

#[tokio::test]
async fn spawn_blocking_with_tracing_returns_join_result() {
    let handle = spawn_blocking_with_tracing(|| 99);
    let value = handle.await.expect("join");
    must_eq(value, 99);
}
