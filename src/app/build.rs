use crate::app::configuration::Settings;
use actix_web::dev::Server;
use anyhow::Result;
use secrecy::SecretString;
use sqlx::PgPool;
use std::net::TcpListener;


pub trait Run: App {
    fn run_until_stopped(
        self
    ) -> impl std::future::Future<Output = Result<(), std::io::Error>> + Send
    where
        Self: Sized + Send
    {
        async { self.server().await }
    }

    fn run<Context: Clone + Send + 'static>(
        listener: TcpListener,
        db_pool: PgPool,
        graph_pool: neo4rs::Graph,
        base_url: String,
        hmac_secret: SecretString,
        redis_uri: SecretString,
        custom_context: Context
    ) -> impl std::future::Future<Output = Result<Server, anyhow::Error>> + Send;
}

pub trait App {
    fn new(port: u16, server: Server) -> Self;
    fn port(&self) -> u16
    where
        Self: Sized;
    fn server(self) -> Server
    where
        Self: Sized;

    fn build<T, C>(config: Settings<T>) -> impl std::future::Future<Output = Result<Self>> + Send
    where
        Self: Sized + Run,
        T: DeriveContext<C> + Send,
        C: Clone + Send + 'static
    {
        async move {
            let connection_pool = config.database.get_connection_pool();

            let graph_pool = config.graphdb.connect().await?;

            let address = format!("{}:{}", config.application.host, config.application.port);

            let listener = TcpListener::bind(address)?;
            let port = listener.local_addr()?.port();

            let context = config.custom.context();

            let server = Self::run(
                listener,
                connection_pool,
                graph_pool,
                config.application.base_url,
                config.application.hmac_secret,
                config.redis_uri,
                context
            )
            .await?;

            Ok(Self::new(port, server))
        }
    }
}

pub trait DeriveContext<C> {
    fn context(&self) -> C
    where
        Self: Sized;
}

pub struct ApplicationBaseUrl(pub String);

#[derive(Clone)]
pub struct HmacSecret(pub SecretString);
