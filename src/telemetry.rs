use anyhow::{Context, Result};
use tokio::task::JoinHandle;
use tracing::Subscriber;
use tracing::subscriber::set_global_default;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::{EnvFilter, Registry, layer::SubscriberExt};

/// Build a [`tracing`] subscriber that emits structured JSON (Bunyan format).
///
/// The `env_filter` string is used as the default log level if the
/// `RUST_LOG` environment variable is not set.  Pass the service name as
/// `name` — it appears as the `name` field in every log record.
///
/// # Examples
///
/// ```no_run
/// use mae::telemetry::{get_subscriber, init_subscriber};
///
/// let subscriber = get_subscriber("my-service".into(), "info".into(), std::io::stdout);
/// init_subscriber(subscriber).expect("failed to init tracing");
/// ```
pub fn get_subscriber<Sink,>(
    name: String,
    env_filter: String,
    sink: Sink,
) -> impl Subscriber + Sync + Send
where
    Sink: for<'a> MakeWriter<'a,> + Sync + Send + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter,),);
    let formatting_layer = BunyanFormattingLayer::new(name, sink,);

    Registry::default().with(env_filter,).with(JsonStorageLayer,).with(formatting_layer,)
}

/// Register a [`tracing`] subscriber as the global default.
///
/// Must be called exactly once per process, before any `tracing` macros are
/// used.  Returns an error if a global subscriber has already been set.
pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync,) -> Result<(),> {
    LogTracer::init().with_context(|| "Failed to set logger",)?;
    set_global_default(subscriber,).with_context(|| "Failed to set subscriber",)?;
    Ok((),)
}

/// Spawn a blocking task that runs within the current [`tracing::Span`].
///
/// Useful when calling CPU-intensive or blocking code from an async context:
/// the span is propagated into the blocking thread so log records emitted
/// inside the closure are correctly nested under the caller's span.
pub fn spawn_blocking_with_tracing<F, R,>(f: F,) -> JoinHandle<R,>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let current_span = tracing::Span::current();
    tokio::task::spawn_blocking(move || current_span.in_scope(f,),)
}
