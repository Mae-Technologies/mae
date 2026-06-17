use crate::context::RequestContext;
use crate::route::response::e500;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{web, FromRequest};
use anyhow::anyhow;

pub enum Scope {
    Public,
    Admin
}

pub(crate) async fn scope<T: 'static + Clone>(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
    target_scope: Scope
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let ctx = req
        .app_data::<web::Data<RequestContext<T>>>()
        .ok_or_else(|| anyhow!("Unable to access Context."))
        .map_err(e500)?;
    let user_id = ctx.session.session_or_err()?;
    next.call(req).await
}
