use std::sync::Arc;

use mae::context::{ContextAccessor, PgContext, RequestContext};
use mae::session::Session;
use mae::testing::must::{MustExpect, must_eq};
use sqlx::PgPool;

fn lazy_pool() -> Arc<PgPool> {
    Arc::new(
        PgPool::connect_lazy("postgres://fake:fake@127.0.0.1:9/fake").must_expect("connect_lazy")
    )
}

#[tokio::test]
async fn pg_context_new_initializes_empty_transaction_slot() {
    let pool = lazy_pool();
    let ctx = PgContext::new(pool).expect("pg context");
    must_eq(ctx.db_pool.is_closed(), false);
}

#[tokio::test]
async fn request_context_exposes_session_user_via_accessor() {
    let pool = lazy_pool();
    let session = Arc::new(Session(Some(42)));
    let custom = Arc::new(());

    let ctx = RequestContext::new(pool, session, custom).await.expect("request context");
    must_eq(ctx.session_user(), Some(42));
    must_eq(ctx.session().session_or_err().expect("session"), 42);
}
