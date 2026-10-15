//! Per-request context: Postgres pool/transaction, session, and service-specific config.
//!
//! Services alias this as `type RequestContext = mae::context::RequestContext<AppContext>`
//! and inject it via `mae::middleware` extractors installed by `#[run_app]`.

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

/// Header the API sends so a downstream service can put the caller's companies
/// on its request context. Absent means no company key. A blank value means
/// the caller has no companies.
pub const SYS_CLIENT_HEADER: &str = "X-Sys-Client";

/// Companies parsed from [`SYS_CLIENT_HEADER`]. `Some([])` matches no rows.
#[derive(Clone, Debug)]
pub struct IncomingSysClients(pub Option<Vec<i32>>);

/// Parse `X-Sys-Client`. An empty header is an empty list. Tokens that are
/// not integers are dropped.
pub fn parse_sys_client_header(raw: &str) -> Vec<i32> {
    if raw.trim().is_empty() {
        return Vec::new();
    }
    raw.split(',').filter_map(|part| part.trim().parse::<i32>().ok()).collect()
}

#[derive(Clone)]
pub struct RequestContext<T: Clone> {
    pub pg_context: Arc<PgContext>,
    pub session: Arc<Session>,
    pub custom: Arc<T>
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
            custom: custom.into_inner()
        })
    }
    pub async fn new(db_pool: Arc<PgPool>, session: Arc<Session>, custom: Arc<T>) -> Result<Self> {
        let pg_context = Arc::new(PgContext::new(db_pool)?);
        Ok(RequestContext { pg_context, session, custom })
    }

    /// Record the companies from the signed-in user's `sys_client` role.
    /// `None` leaves the request unrestricted.
    pub fn set_sys_clients(&self, companies: Option<Vec<i32>>) {
        self.pg_context.set_sys_clients(companies);
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
        if let Some(incoming) = req.extensions().get::<IncomingSysClients>() {
            pg_ctx.set_sys_clients(incoming.0.clone());
        }

        let ctx = RequestContext { pg_context: Arc::new(pg_ctx), session, custom };

        ready(Ok(ctx))
    }
}

#[derive(Clone)]
pub struct PgContext {
    pub tx: Arc<Mutex<Option<PgTransaction<'static>>>>,
    pub db_pool: Arc<PgPool>,
    /// `None` is unrestricted. `Some` is the signed-in user's companies.
    /// Shared across clones of this context.
    sys_clients: Arc<StdMutex<Option<Vec<i32>>>>
}

impl PgContext {
    pub fn new(db_pool: Arc<PgPool>) -> Result<Self> {
        let tx = Arc::new(Mutex::new(None));
        Ok(PgContext { tx, db_pool, sys_clients: Arc::new(StdMutex::new(None)) })
    }

    pub fn set_sys_clients(&self, companies: Option<Vec<i32>>) {
        *self.sys_clients.lock().unwrap_or_else(|err| err.into_inner()) = companies;
    }

    pub fn sys_clients(&self) -> Option<Vec<i32>> {
        self.sys_clients.lock().unwrap_or_else(|err| err.into_inner()).clone()
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
        self.pg_context.sys_clients()
    }
}

#[cfg(test)]
mod tests {
    use super::{PgContext, parse_sys_client_header};
    use crate::testing::must::{MustExpect, must_be_true};
    use std::sync::Arc;

    #[test]
    fn parse_sys_client_header_keeps_integers() {
        must_be_true(parse_sys_client_header("3, 4") == vec![3, 4]);
        must_be_true(parse_sys_client_header("").is_empty());
        must_be_true(parse_sys_client_header("  ").is_empty());
        must_be_true(parse_sys_client_header("3,nope,4") == vec![3, 4]);
    }

    #[tokio::test]
    async fn pg_context_stores_company_list() {
        let pool = sqlx::PgPool::connect_lazy("postgres://fake:fake@127.0.0.1:9/fake")
            .must_expect("lazy pool");
        let pg = PgContext::new(Arc::new(pool)).must_expect("pg context");
        must_be_true(pg.sys_clients().is_none());
        pg.set_sys_clients(Some(vec![3, 4]));
        must_be_true(pg.sys_clients() == Some(vec![3, 4]));
        pg.set_sys_clients(Some(vec![]));
        must_be_true(pg.sys_clients() == Some(vec![]));
        pg.set_sys_clients(None);
        must_be_true(pg.sys_clients().is_none());
    }
}
