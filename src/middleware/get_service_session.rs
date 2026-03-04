use crate::middleware::get_session;
use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;

pub async fn get_service_session(
    req: ServiceRequest,
    next: Next<impl MessageBody,>,
) -> Result<ServiceResponse<impl MessageBody,>, actix_web::Error,> {
    // Service-to-service session handling currently mirrors standard session middleware.
    // Kept separate for explicit call sites in microservice run wiring.
    get_session(req, next,).await
}
