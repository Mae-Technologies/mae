use crate::session::Session;
use actix_web::web::{Data, ReqData};
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone,)]
pub struct RequestContext<T: Clone,> {
    pub db_pool: Arc<PgPool,>,
    pub session: Session,
    pub custom: Arc<T,>,
}

impl<T: Clone,> RequestContext<T,> {
    pub fn new(db_pool: Data<PgPool,>, session: ReqData<Session,>, custom: Data<T,>,) -> Self {
        RequestContext {
            db_pool: db_pool.into_inner(),
            session: session.into_inner(),
            custom: custom.into_inner(),
        }
    }
}

pub trait ContextAccessor {
    fn db_pool(&self,) -> &PgPool;
    // TODO: implement the other property accessor functions (ie Session, CustomContext)
}

impl<T: Clone,> ContextAccessor for RequestContext<T,> {
    fn db_pool(&self,) -> &PgPool {
        &self.db_pool
    }
}
