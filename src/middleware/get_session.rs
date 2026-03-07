use crate::error_response::e500;
use crate::session::TypedSession;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::error::InternalError;
use actix_web::middleware::Next;
use actix_web::{FromRequest, HttpMessage, HttpResponse};

pub async fn get_session(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let session = {
        let (http_req, payload) = req.parts_mut();
        TypedSession::from_request(http_req, payload).await
    }?;

    match session.get_session().map_err(e500)? {
        Some(session_data) => {
            req.extensions_mut().insert(session_data);
            next.call(req).await
        }
        None => {
            let resp = HttpResponse::Unauthorized().finish();
            let e = anyhow::anyhow!("Unauthorized.");
            Err(InternalError::from_response(e, resp).into())
        }
    }
}
