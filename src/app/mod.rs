//! Application lifecycle helpers for Mae micro-services.
//!
//! This module provides the three building blocks every Mae service needs at
//! startup:
//!
//! - **Configuration** ([`configuration`]) — deserialises YAML config files
//!   and environment overrides into a typed [`configuration::Settings`].
//! - **Builder** ([`build`]) — the [`build::App`] and [`build::Run`] traits
//!   that services implement to wire up their Actix-Web server, database pool,
//!   Redis session store, and custom context.
//! - **Runner** ([`run`]) — the top-level [`run`] async function that
//!   initialises telemetry, loads config, and drives the server to completion.
//!
//! Most services only need [`prelude`], which re-exports the complete set of
//! types required in a typical `main.rs`.

// TODO: Im pretty sure the crate::aop::app is not being used at all. -- it has been removed, but
// if something errors when using this crate, look into it.
#[allow(clippy::module_inception)]
pub mod app;

/// Re-exports [`redis_session`] and [`session_middleware`] from the inner `app` module
/// so that microservices can reach them via `mae::app::{redis_session, session_middleware}`
/// without needing to know the internal module layout.
///
/// - [`redis_session`] — async constructor that connects to Redis and returns a
///   [`RedisSessionStore`] ready to be passed to [`session_middleware`].
/// - [`session_middleware`] — builds the [`SessionMiddleware`] that must be registered
///   with every Actix-Web app that needs persistent user sessions.
pub use app::{cors_middleware, redis_session, session_middleware};
pub mod build;
pub mod configuration;
mod run;
pub use run::*;

pub mod prelude {

    pub use crate::app::build::{App, ApplicationBaseUrl, HmacSecret, Run};
    pub use crate::app::run::run;
    /// Both session helpers are re-exported here so microservice `main.rs` files have a
    /// single import path: `use mae::app::prelude::{get_service_session, get_session}`.
    pub use crate::middleware::{get_service_session, get_session};
    pub use mae_macros::*;

    pub use actix_cors::Cors;
    pub use actix_web::dev::Server;
    pub use actix_web::middleware::from_fn;
    pub use actix_web::{App as ActixWebApp, HttpServer, web};
    pub use secrecy::SecretString;
    pub use sqlx::PgPool;
    pub use std::net::TcpListener;
    pub use tracing_actix_web::TracingLogger;
}
