// use crate::context::{PgContext, RequestContext};
use crate::context::RequestContext;
use crate::route::response::e500;
use crate::session::SessionHandler;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{web, FromRequest, HttpMessage};
use anyhow::anyhow;
use sqlx::PgPool;
use std::sync::Arc;

// WARNING: This function currently doesn't work... although it compiles.
// route 500's with a 'missing expected request extension data' message
pub async fn get_context<T: 'static + Clone>(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session = {
        let (http_req, payload) = req.parts_mut();
        SessionHandler::from_request(http_req, payload).await
    }?;

    let session_data = session.get_session().map_err(e500)?;
    let db_pool = Arc::clone(
        &req.app_data::<web::Data<PgPool>>()
            .ok_or_else(|| anyhow!("Unable to access PgPool."))
            .map_err(e500)?
            .clone()
            .into_inner()
    );

    let custom = Arc::clone(
        &req.app_data::<web::Data<T>>()
            .ok_or_else(|| anyhow!("Unable to access Context."))
            .map_err(e500)?
            .clone()
            .into_inner()
    );
    let ctx = RequestContext::new(db_pool, Arc::new(session_data), custom).await.map_err(e500)?;
    req.extensions_mut().insert(ctx);
    next.call(req).await
}
