// TODO: Im pretty sure the crate::aop::app is not being used at all. -- it has been removed, but
// if something errors when using this crate, look into it.
mod app;
pub mod build;
pub mod configuration;
mod run;
pub use app::*;
pub use run::*;

pub mod prelude {
    pub use crate::app::build::{App, ApplicationBaseUrl, HmacSecret, Run};
    pub use crate::app::run::run;
    pub use crate::middleware::{get_service_session, get_session};
    pub use mae_macros::*;

    pub use actix_web::dev::Server;
    pub use actix_web::middleware::from_fn;
    pub use actix_web::{App as ActixWebApp, HttpServer, web};
    pub use secrecy::SecretString;
    pub use sqlx::PgPool;
    pub use std::net::TcpListener;
    pub use tracing_actix_web::TracingLogger;
}
