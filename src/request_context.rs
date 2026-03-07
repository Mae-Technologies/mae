use crate::session::Session;
use actix_web::web::{Data, ReqData};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct RequestContext<T: Clone> {
    pub db_pool: Arc<PgPool>,
    pub session: Session,
    pub custom: Arc<T>
}

impl<T: Clone> RequestContext<T> {
    pub fn new(db_pool: Data<PgPool>, session: ReqData<Session>, custom: Data<T>) -> Self {
        RequestContext {
            db_pool: db_pool.into_inner(),
            session: session.into_inner(),
            custom: custom.into_inner()
        }
    }
}

// TODO: ContextAccessor may not be required
pub trait ContextAccessor {
    fn db_pool(&self) -> &PgPool;
    fn session(&self) -> &Session;
    fn session_user(&self) -> &i32;
}

impl<T: Clone> ContextAccessor for RequestContext<T> {
    fn db_pool(&self) -> &PgPool {
        &self.db_pool
    }
    fn session(&self) -> &Session {
        &self.session
    }
    fn session_user(&self) -> &i32 {
        &self.session.user_id
    }
}
