//! Per-request context: Postgres pool/transaction, session, and service-specific config.
//!
//! Services alias this as `type RequestContext = mae::context::RequestContext<AppContext>`
//! and inject it via `mae::middleware` extractors installed by `#[run_app]`.

use crate::grants::{Grants, RequestGrants};
use crate::session::Session;
use actix_web::web::{Data, ReqData};
use actix_web::{
    Error, FromRequest, HttpMessage, HttpRequest,
    dev::Payload,
    error::{ErrorInternalServerError, ErrorUnauthorized},
    web
};
use anyhow::{Context, Result};
use futures_util::future::{Ready, ready};
use sqlx::{PgPool, PgTransaction};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::{Mutex, MutexGuard};

#[derive(Clone)]
pub struct RequestContext<T: Clone> {
    pub pg_context: Arc<PgContext>,
    pub session: Arc<Session>,
    pub custom: Arc<T>,
    /// Copied from the middleware extension when the context is built.
    /// There is no setter. Replacing `pg_context` does not clear this.
    grants: Arc<StdMutex<Option<Grants>>>
}

impl<T: Clone> RequestContext<T> {
    pub async fn from_request(
        db_pool: Data<PgPool>,
        session: ReqData<Session>,
        custom: Data<T>
    ) -> Result<Self> {
        let pg_context = Arc::new(PgContext::new(db_pool.into_inner())?);
        Ok(RequestContext {
            pg_context,
            session: Arc::new(session.into_inner()),
            custom: custom.into_inner(),
            grants: Arc::new(StdMutex::new(None))
        })
    }
    pub async fn new(db_pool: Arc<PgPool>, session: Arc<Session>, custom: Arc<T>) -> Result<Self> {
        let pg_context = Arc::new(PgContext::new(db_pool)?);
        Ok(RequestContext { pg_context, session, custom, grants: Arc::new(StdMutex::new(None)) })
    }
}
impl<T> FromRequest for RequestContext<T>
where
    T: Clone + 'static
{
    type Error = Error;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        let db_pool = match req.app_data::<web::Data<PgPool>>() {
            Some(pool) => pool.clone().into_inner(),
            None => {
                return ready(Err(ErrorInternalServerError("missing PgPool app data")));
            }
        };

        let custom = match req.app_data::<web::Data<T>>() {
            Some(custom) => custom.clone().into_inner(),
            None => {
                return ready(Err(ErrorInternalServerError("missing custom app data")));
            }
        };

        let session = match req.extensions().get::<Session>().cloned() {
            Some(session) => Arc::new(session),
            None => {
                return ready(Err(ErrorUnauthorized("missing request session")));
            }
        };

        let pg_ctx = match PgContext::new(db_pool) {
            Ok(pg_ctx) => pg_ctx,
            Err(e) => return ready(Err(ErrorInternalServerError(e)))
        };
        let grants = req.extensions().get::<RequestGrants>().map(|incoming| incoming.0.clone());

        let ctx = RequestContext {
            pg_context: Arc::new(pg_ctx),
            session,
            custom,
            grants: Arc::new(StdMutex::new(grants))
        };

        ready(Ok(ctx))
    }
}

#[derive(Clone)]
pub struct PgContext {
    pub tx: Arc<Mutex<Option<PgTransaction<'static>>>>,
    pub db_pool: Arc<PgPool>
}

impl PgContext {
    pub fn new(db_pool: Arc<PgPool>) -> Result<Self> {
        let tx = Arc::new(Mutex::new(None));
        Ok(PgContext { tx, db_pool })
    }

    pub async fn with_tx<R, F>(&self, f: F) -> Result<R>
    where
        F: for<'tx> FnOnce(&'tx mut PgTransaction) -> TxFuture<'tx, R>
    {
        let mut guard = self.tx.lock().await;

        if guard.is_none() {
            let tx =
                self.db_pool.begin().await.with_context(|| "unable to create pg-transaction")?;

            *guard = Some(tx);
        }

        let tx = guard.as_mut().context("transaction should be initialized")?;

        f(tx).await
    }

    pub async fn commit(&self) -> Result<()> {
        let mut g = self.tx.lock().await;

        if let Some(tx) = g.take() {
            tx.commit().await?;
        }
        Ok(())
    }
}

pub struct PgTxGuard<'g, 't> {
    guard: MutexGuard<'g, Option<PgTransaction<'t>>>
}

impl<'g, 't> Deref for PgTxGuard<'g, 't> {
    type Target = PgTransaction<'t>;

    fn deref(&self) -> &Self::Target {
        match self.guard.as_ref() {
            Some(tx) => tx,
            None => panic!("transaction should be initialized")
        }
    }
}

impl<'g, 't> DerefMut for PgTxGuard<'g, 't> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self.guard.as_mut() {
            Some(tx) => tx,
            None => panic!("transaction should be initialized")
        }
    }
}

pub type TxFuture<'tx, R> = Pin<Box<dyn Future<Output = Result<R>> + Send + 'tx>>;

pub trait ContextAccessor {
    fn pg_context(&self) -> &PgContext;

    fn session(&self) -> &Session;

    fn session_user(&self) -> Option<i32> {
        self.session().0
    }

    /// `None`: no company predicate. `Some`: companies on the signed-in user's
    /// `sys_client` role. An empty list matches no rows.
    fn sys_clients(&self) -> Option<Vec<i32>> {
        None
    }
}

impl<C: Clone> ContextAccessor for RequestContext<C> {
    fn pg_context(&self) -> &PgContext {
        &self.pg_context
    }

    fn session(&self) -> &Session {
        &self.session
    }

    fn sys_clients(&self) -> Option<Vec<i32>> {
        self.grants
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .as_ref()
            .and_then(|grants| grants.sys_client.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::RequestContext;
    use crate::context::ContextAccessor;
    use crate::grants::{Grants, RequestGrants};
    use crate::session::Session;
    use crate::testing::must::{MustExpect, must_be_true, must_eq};
    use actix_web::FromRequest;
    use actix_web::HttpMessage;
    use actix_web::test::TestRequest;
    use actix_web::web::Data;
    use std::sync::Arc;

    async fn companies_from(grants: Option<Grants>) -> Option<Vec<i32>> {
        let pool = sqlx::PgPool::connect_lazy("postgres://fake:fake@127.0.0.1:9/fake")
            .must_expect("lazy pool");
        let srv =
            TestRequest::get().app_data(Data::new(pool)).app_data(Data::new(())).to_srv_request();
        srv.extensions_mut().insert(Session(Some(7)));
        if let Some(grants) = grants {
            srv.extensions_mut().insert(RequestGrants(grants));
        }
        let (req, _payload) = srv.into_parts();
        let ctx = RequestContext::<()>::extract(&req).await.must_expect("ctx");
        ctx.sys_clients()
    }

    /// The snapshot is whatever middleware put on the request. `new` has none.
    #[tokio::test]
    async fn request_context_reads_the_middleware_snapshot() {
        must_be_true(companies_from(None).await.is_none());
        must_be_true(companies_from(Some(Grants::open())).await.is_none());
        must_eq(companies_from(Some(Grants::companies(vec![4, 3, 3]))).await, Some(vec![3, 4]));
        must_eq(companies_from(Some(Grants::companies(vec![]))).await, Some(vec![]));

        let pool = sqlx::PgPool::connect_lazy("postgres://fake:fake@127.0.0.1:9/fake")
            .must_expect("lazy pool");
        let built = RequestContext::new(Arc::new(pool), Arc::new(Session(None)), Arc::new(()))
            .await
            .must_expect("new");
        must_be_true(built.sys_clients().is_none());
    }
}
