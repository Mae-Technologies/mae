use crate::route::response::e500;
use crate::session::SessionHandler;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{FromRequest, HttpMessage};

pub async fn get_session(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let typed_session = {
        let (http_req, payload) = req.parts_mut();
        SessionHandler::from_request(http_req, payload).await
    }?;

    // The cookie only has a user id. `get_context` inserts `Session<T::Snapshot>`
    // for the application snapshot. This check rejects a corrupt `user_id`.
    let _session = typed_session.get_session::<()>().map_err(e500)?;
    req.extensions_mut().insert(typed_session);
    next.call(req).await
}
