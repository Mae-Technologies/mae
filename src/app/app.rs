use actix_session::SessionMiddleware;
use actix_session::config::{PersistentSession, TtlExtensionPolicy};
use actix_session::storage::RedisSessionStore;
use actix_web::cookie::Key;
use secrecy::ExposeSecret;
use secrecy::SecretString;

pub async fn redis_session(redis_uri: SecretString,) -> Result<RedisSessionStore, anyhow::Error,> {
    RedisSessionStore::new(redis_uri.expose_secret(),).await
}

pub fn session_middleware(
    hmac_secret: SecretString,
    redis_store: RedisSessionStore,
) -> SessionMiddleware<RedisSessionStore,> {
    SessionMiddleware::builder(
        redis_store.clone(),
        Key::from(hmac_secret.expose_secret().as_bytes(),),
    )
    .session_lifecycle(
        PersistentSession::default()
            .session_ttl_extension_policy(TtlExtensionPolicy::OnEveryRequest,),
    )
    .cookie_http_only(false,)
    .cookie_secure(false,)
    .build()
}
