use actix_session::SessionMiddleware;
use actix_session::config::{PersistentSession, TtlExtensionPolicy};
use actix_session::storage::RedisSessionStore;
use actix_web::cookie::Key;
use secrecy::ExposeSecret;
use secrecy::SecretString;

/// Connect to Redis and create a session store.
///
/// Must be called once during microservice startup before constructing the Actix-Web app.
/// Pass the returned store to [`session_middleware`].
///
/// # Parameters
/// - `redis_uri` — connection URI for the Redis instance (e.g. `redis://127.0.0.1:6379`).
///   Wrapped in [`SecretString`] so the URI (which may contain credentials) is never
///   accidentally printed to logs.
///
/// # Errors
/// Returns an error if the Redis connection cannot be established at startup.
pub async fn redis_session(redis_uri: SecretString) -> Result<RedisSessionStore, anyhow::Error> {
    RedisSessionStore::new(redis_uri.expose_secret()).await
}

/// Build the Actix-Web session middleware backed by Redis.
///
/// Call this once per microservice and register the result with `.wrap(…)` on the
/// Actix-Web [`App`](actix_web::App).
///
/// Session behaviour:
/// - **Persistent sessions** with TTL extended on every request (`OnEveryRequest`),
///   so active users are never unexpectedly logged out.
/// - `cookie_http_only(false)` — allows JavaScript to read the session cookie (required
///   by the Angular front-end for token refresh flows).
/// - `cookie_secure(false)` — permits HTTP in development; set to `true` in production
///   behind TLS.
///
/// # Parameters
/// - `hmac_secret` — secret used to sign the session cookie. Must be at least 64 bytes;
///   loaded from configuration and wrapped in [`SecretString`] to prevent accidental
///   logging.
/// - `redis_store` — the store returned by [`redis_session`].
pub fn session_middleware(
    hmac_secret: SecretString,
    redis_store: RedisSessionStore
) -> SessionMiddleware<RedisSessionStore> {
    SessionMiddleware::builder(
        redis_store.clone(),
        Key::from(hmac_secret.expose_secret().as_bytes())
    )
    .session_lifecycle(
        PersistentSession::default()
            .session_ttl_extension_policy(TtlExtensionPolicy::OnEveryRequest)
    )
    .cookie_http_only(false)
    .cookie_secure(false)
    .build()
}
