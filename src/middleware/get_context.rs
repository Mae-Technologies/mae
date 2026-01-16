use crate::error_response::e500;
use crate::request_context::RequestContext;
use crate::session::TypedSession;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::middleware::Next;
use actix_web::{FromRequest, HttpMessage, HttpResponse, web};
use anyhow::anyhow;
use sqlx::PgPool;
use std::sync::Arc;

// WARNING: This function currently doesn't work... although it compiles.
// route 500's with a 'missing expected request extension data' message
pub async fn get_context<T: 'static + Clone>(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session = {
        let (http_req, payload) = req.parts_mut();
        TypedSession::from_request(http_req, payload).await
    }?;

    match session.get_session().map_err(e500)? {
        Some(session) => {
            let db_pool = Arc::clone(
                &req.app_data::<web::Data<PgPool>>()
                    .ok_or_else(|| anyhow!("Unable to access PgPool."))
                    .map_err(e500)?
                    .clone()
                    .into_inner(),
            );

            let custom = Arc::clone(
                &req.app_data::<web::Data<T>>()
                    .ok_or_else(|| anyhow!("Unable to access Context."))
                    .map_err(e500)?
                    .clone()
                    .into_inner(),
            );
            req.extensions_mut().insert(RequestContext {
                db_pool,
                custom,
                session,
            });
            next.call(req).await
        }
        None => {
            let resp = HttpResponse::Unauthorized().finish();
            let e = anyhow::anyhow!("Unauthorized.");
            Err(InternalError::from_response(e, resp).into())
        }
    }
}
