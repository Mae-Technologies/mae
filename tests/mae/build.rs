// NOTE: commented out sections are for a WIP related to the get_context middleware
use mae::request_context::RequestContext;
use mae::session::Session;
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};

fn connect() -> PgPool {
    PgPoolOptions::new().connect_lazy_with(
        PgConnectOptions::new()
            .host("127.0.0.1")
            .port(2345)
            .password("password")
            .username("postgres")
            .ssl_mode(PgSslMode::Prefer)
            .database("test_mae"),
    )
}

pub async fn get_context<T: Clone>(custom_context: T) -> Result<RequestContext<T>, sqlx::Error> {
    let conn = connect();

    let ctx = RequestContext::<T> {
        db_pool: conn.into(),
        session: Session { user_id: 1 },
        custom: custom_context.into(),
    };
    Ok(ctx)
}
