use crate::session::Session;
use actix_web::web::{Data, ReqData};
use sqlx::{PgPool, PgTransaction};
use std::sync::Arc;

#[derive(Clone)]
pub struct RequestContext<'p, T: Clone> {
    pub db_pool: Arc<PgPool>,
    pub db_trx: Arc<Option<PgTransaction<'p>>>,
    pub session: Session,
    pub custom: Arc<T>
}

impl<'a, T: Clone> RequestContext<'a, T> {
    pub fn new(db_pool: Data<PgPool>, session: ReqData<Session>, custom: Data<T>) -> Self {
        RequestContext {
            db_pool: db_pool.into_inner(),
            db_trx: Arc::new(None),
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
    fn db_trx(&self) -> &Option<PgTransaction>;
}

impl<'a, T: Clone> ContextAccessor for RequestContext<'a, T> {
    fn db_pool(&self) -> &PgPool {
        &self.db_pool
    }
    fn db_trx(&self) -> &Option<PgTransaction<'a>> {
        &self.db_trx
    }
    fn session(&self) -> &Session {
        &self.session
    }
    fn session_user(&self) -> &i32 {
        &self.session.user_id
    }
}
