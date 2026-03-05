//! RabbitMQ container singleton + per-test vhost isolation.
//!
//! # Isolation strategy
//! RabbitMQ virtual hosts (vhosts) provide complete namespace isolation.
//! Each test scope owns a unique vhost (`mae_test_<uuid>`).  On teardown
//! the vhost is deleted via the management HTTP API, removing all exchanges,
//! queues, and bindings within it.

use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::rabbitmq::RabbitMq;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::testing::must::testcontainers_enabled;

use super::MaeContainer;

/// Default AMQP port.
pub const RABBITMQ_AMQP_PORT: u16 = 5672;
/// Default management HTTP port.
pub const RABBITMQ_MGMT_PORT: u16 = 15672;
/// Default guest credentials used by the official image.
const DEFAULT_USER: &str = "guest";
const DEFAULT_PASS: &str = "guest";

// ── Singleton state ───────────────────────────────────────────────────────────

pub struct Inner {
    pub container: ContainerAsync<RabbitMq,>,
    pub amqp_port: u16,
    pub mgmt_port: u16,
}

static SINGLETON: OnceLock<Mutex<Option<Inner,>,>,> = OnceLock::new();

fn rabbitmq_mutex() -> &'static Mutex<Option<Inner,>,> {
    SINGLETON.get_or_init(|| Mutex::new(None,),)
}

// ── Public isolation scope ────────────────────────────────────────────────────

/// Per-test isolation scope for RabbitMQ.
pub struct RabbitMqScope {
    /// Virtual host name assigned to this scope (e.g. `mae_test_<uuid>`).
    pub vhost: String,
    pub amqp_url: String,
    pub mgmt_base_url: String,
}

impl RabbitMqScope {
    /// Returns the AMQP URL scoped to this test's vhost.
    pub fn amqp_url(&self,) -> &str {
        &self.amqp_url
    }

    /// Returns the vhost name.
    pub fn vhost(&self,) -> &str {
        &self.vhost
    }

    /// Delete this vhost via the RabbitMQ management API.
    ///
    /// This removes all exchanges, queues, and bindings within the vhost.
    pub async fn delete_vhost(&self,) -> Result<(),> {
        delete_vhost(&self.mgmt_base_url, &self.vhost,).await
    }
}

// ── MaeContainer impl ─────────────────────────────────────────────────────────

/// Zero-sized handle for the RabbitMQ container singleton.
pub struct RabbitMqContainer;

impl MaeContainer for RabbitMqContainer {
    type Scope = RabbitMqScope;

    async fn start() -> Option<(),> {
        rabbitmq_singleton().await.lock().await.as_ref().map(|_| (),)
    }

    async fn scope() -> Result<RabbitMqScope,> {
        spawn_scoped_vhost().await
    }

    async fn teardown() {
        teardown().await;
    }
}

// ── Free functions ────────────────────────────────────────────────────────────

/// Returns `(amqp_url, amqp_port)` for the shared RabbitMQ container.
///
/// The URL uses the default `/` vhost.  For per-test isolation, prefer
/// [`spawn_scoped_vhost`].
///
/// # Errors
/// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled.
pub async fn get_rabbitmq_amqp() -> Result<(String, u16,),> {
    ensure_started().await?;
    let guard = rabbitmq_mutex().lock().await;
    let c = guard.as_ref().context("rabbitmq container is not running — this is a bug",)?;
    let url = amqp_url(c.amqp_port, "/",);
    Ok((url, c.amqp_port,),)
}

/// Returns the RabbitMQ singleton, starting the container if needed.
pub async fn rabbitmq_singleton() -> &'static Mutex<Option<Inner,>,> {
    let _ = ensure_started().await;
    rabbitmq_mutex()
}

/// Create a fresh per-test vhost scope via the management HTTP API.
///
/// # Errors
/// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled, or when the
/// management API call fails.
pub async fn spawn_scoped_vhost() -> Result<RabbitMqScope,> {
    ensure_started().await?;

    let (amqp_port, mgmt_port,) = {
        let guard = rabbitmq_mutex().lock().await;
        let c = guard.as_ref().context("rabbitmq container is not running — this is a bug",)?;
        (c.amqp_port, c.mgmt_port,)
    };

    let vhost_id = Uuid::new_v4().to_string().replace('-', "");
    let vhost = format!("mae_test_{vhost_id}");
    let mgmt_base_url = format!("http://localhost:{mgmt_port}");

    // Create the vhost via management HTTP API.
    create_vhost(&mgmt_base_url, &vhost,).await?;

    // Grant guest full permissions on the new vhost.
    set_permissions(&mgmt_base_url, &vhost, DEFAULT_USER,).await?;

    let url = amqp_url(amqp_port, &vhost,);
    Ok(RabbitMqScope { vhost, amqp_url: url, mgmt_base_url, },)
}

/// Stop the RabbitMQ container and reset the singleton.
pub async fn teardown() {
    let mut guard = rabbitmq_mutex().lock().await;
    if let Some(c,) = guard.take() {
        let _ = c.container.stop().await;
        drop(c,);
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

async fn ensure_started() -> Result<(),> {
    if !testcontainers_enabled() {
        bail!(
            "testcontainers disabled — set MAE_TESTCONTAINERS=1 to run RabbitMQ tests. \
             In Concourse, ensure the task has Docker socket access (DinD)."
        );
    }

    let mut guard = rabbitmq_mutex().lock().await;
    if guard.is_none() {
        let container: ContainerAsync<RabbitMq,> =
            RabbitMq::default().start().await.context("failed to start RabbitMQ container",)?;

        let amqp_port = container
            .get_host_port_ipv4(RABBITMQ_AMQP_PORT,)
            .await
            .context("failed to get RabbitMQ AMQP port",)?;

        let mgmt_port = container
            .get_host_port_ipv4(RABBITMQ_MGMT_PORT,)
            .await
            .context("failed to get RabbitMQ management port",)?;

        *guard = Some(Inner { container, amqp_port, mgmt_port, },);
    }
    Ok((),)
}

fn amqp_url(port: u16, vhost: &str,) -> String {
    // Vhost `/` must be percent-encoded as `%2f` in AMQP URLs.
    let encoded_vhost = if vhost == "/" { "%2f".to_string() } else { vhost.to_string() };
    format!("amqp://{DEFAULT_USER}:{DEFAULT_PASS}@localhost:{port}/{encoded_vhost}")
}

async fn create_vhost(mgmt_base_url: &str, vhost: &str,) -> Result<(),> {
    let url = format!("{mgmt_base_url}/api/vhosts/{vhost}");
    let client = reqwest::Client::new();
    let resp = client
        .put(&url,)
        .basic_auth(DEFAULT_USER, Some(DEFAULT_PASS,),)
        .header("content-type", "application/json",)
        .body("{}",)
        .send()
        .await
        .with_context(|| format!("failed to create RabbitMQ vhost `{vhost}`"),)?;

    anyhow::ensure!(
        resp.status().is_success() || resp.status().as_u16() == 204,
        "RabbitMQ management API returned {} when creating vhost `{}`",
        resp.status(),
        vhost
    );
    Ok((),)
}

async fn set_permissions(mgmt_base_url: &str, vhost: &str, user: &str,) -> Result<(),> {
    let url = format!("{mgmt_base_url}/api/permissions/{vhost}/{user}");
    let client = reqwest::Client::new();
    let resp = client
        .put(&url,)
        .basic_auth(DEFAULT_USER, Some(DEFAULT_PASS,),)
        .header("content-type", "application/json",)
        .body(r#"{"configure":".*","write":".*","read":".*"}"#,)
        .send()
        .await
        .with_context(|| format!("failed to set permissions on vhost `{vhost}`"),)?;

    anyhow::ensure!(
        resp.status().is_success() || resp.status().as_u16() == 204,
        "RabbitMQ management API returned {} when setting permissions on `{}`",
        resp.status(),
        vhost
    );
    Ok((),)
}

async fn delete_vhost(mgmt_base_url: &str, vhost: &str,) -> Result<(),> {
    let url = format!("{mgmt_base_url}/api/vhosts/{vhost}");
    let client = reqwest::Client::new();
    let resp = client
        .delete(&url,)
        .basic_auth(DEFAULT_USER, Some(DEFAULT_PASS,),)
        .send()
        .await
        .with_context(|| format!("failed to delete RabbitMQ vhost `{vhost}`"),)?;

    anyhow::ensure!(
        resp.status().is_success() || resp.status().as_u16() == 204,
        "RabbitMQ management API returned {} when deleting vhost `{}`",
        resp.status(),
        vhost
    );
    Ok((),)
}
