//! Singleton RabbitMQ testcontainer with per-test vhost isolation.
//!
//! ## Usage
//!
//! ```rust,no_run
//! use mae::testing::containers::rabbitmq::{get_rabbitmq_amqp, IsolatedRabbitMq};
//!
//! #[tokio::test]
//! async fn my_test() {
//!     let iso = IsolatedRabbitMq::create().await.expect("rabbitmq must be running");
//!     // iso.amqp_url() — scoped to the unique vhost for this test
//!     // Call iso.delete().await to remove the vhost after the test.
//! }
//! ```
//!
//! ## Per-test isolation
//!
//! Each test gets its own RabbitMQ virtual host (vhost), created via the
//! management HTTP API (port 15672).  Vhosts are fully isolated: queues,
//! exchanges, and bindings in one vhost are invisible to all others.
//!
//! [`IsolatedRabbitMq::create`] creates the vhost.
//! [`IsolatedRabbitMq::delete`] removes it and all its resources.
//!
//! ## Docker-in-Docker notes
//!
//! The management API is accessed via `localhost` on the mapped host port.
//! In Concourse DinD this works as long as the task container can reach the
//! Docker-managed bridge network (default setup).

use std::sync::OnceLock;

use anyhow::{Context, Result, bail};
use testcontainers::{ContainerAsync, runners::AsyncRunner};
use testcontainers_modules::rabbitmq::RabbitMq;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::testing::must::testcontainers_enabled;

/// Default AMQP port.
pub const RABBITMQ_AMQP_PORT: u16 = 5672;
/// Default management HTTP port.
pub const RABBITMQ_MGMT_PORT: u16 = 15672;
/// Default guest credentials used by the official image.
const DEFAULT_USER: &str = "guest";
const DEFAULT_PASS: &str = "guest";

// ── Singleton state ──────────────────────────────────────────────────────────

/// Internal container state.
pub struct RabbitMqContainer {
    pub container: ContainerAsync<RabbitMq,>,
    pub amqp_port: u16,
    pub mgmt_port: u16,
}

static RABBITMQ: OnceLock<Mutex<Option<RabbitMqContainer,>,>,> = OnceLock::new();

fn rabbitmq_mutex() -> &'static Mutex<Option<RabbitMqContainer,>,> {
    RABBITMQ.get_or_init(|| Mutex::new(None,),)
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Returns `(amqp_url, amqp_port)` for the shared RabbitMQ container.
///
/// The URL uses the default `/` vhost.  For per-test isolation, prefer
/// [`IsolatedRabbitMq::create`].
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

/// Stops the RabbitMQ container and resets the singleton.
///
/// The next call to a getter will start a fresh container.
pub async fn teardown() {
    let mut guard = rabbitmq_mutex().lock().await;
    if let Some(c,) = guard.take() {
        let _ = c.container.stop().await;
        drop(c,);
    }
}

// ── Per-test isolation ───────────────────────────────────────────────────────

/// A unique RabbitMQ virtual host for one test.
///
/// Created via the management HTTP API.  Deleting this handle removes
/// the vhost and all queues/exchanges/bindings within it.
pub struct IsolatedRabbitMq {
    pub vhost: String,
    pub amqp_url: String,
    pub mgmt_base_url: String,
    _amqp_port: u16,
    _mgmt_port: u16,
}

impl IsolatedRabbitMq {
    /// Creates a new vhost and returns an isolation handle.
    ///
    /// The vhost is created via the RabbitMQ management HTTP API.
    ///
    /// # Errors
    /// Returns `Err` when `MAE_TESTCONTAINERS` is not enabled, or when the
    /// management API call fails.
    pub async fn create() -> Result<Self,> {
        ensure_started().await?;

        let (amqp_port, mgmt_port,) = {
            let guard = rabbitmq_mutex().lock().await;
            let c = guard.as_ref().context("rabbitmq container is not running — this is a bug",)?;
            (c.amqp_port, c.mgmt_port,)
        };

        let vhost_id = Uuid::new_v4().to_string().replace('-', "",);
        let vhost = format!("mae_test_{vhost_id}");

        let mgmt_base_url = format!("http://localhost:{mgmt_port}");

        // Create the vhost via management HTTP API.
        create_vhost(&mgmt_base_url, &vhost,).await?;

        // Grant guest full permissions on the new vhost.
        set_permissions(&mgmt_base_url, &vhost, DEFAULT_USER,).await?;

        let amqp_url = amqp_url(amqp_port, &vhost,);

        Ok(Self { vhost, amqp_url, mgmt_base_url, _amqp_port: amqp_port, _mgmt_port: mgmt_port, },)
    }

    /// Returns the AMQP URL scoped to this test's vhost.
    ///
    /// Example: `amqp://guest:guest@localhost:32768/mae_test_abc123`
    pub fn amqp_url(&self,) -> &str {
        &self.amqp_url
    }

    /// Returns the vhost name.
    pub fn vhost(&self,) -> &str {
        &self.vhost
    }

    /// Returns the management API base URL, e.g. `"http://localhost:15672"`.
    pub fn mgmt_base_url(&self,) -> &str {
        &self.mgmt_base_url
    }

    /// Deletes this vhost and all its resources via the management HTTP API.
    ///
    /// # Errors
    /// Returns `Err` if the management API call fails.
    pub async fn delete(self,) -> Result<(),> {
        delete_vhost(&self.mgmt_base_url, &self.vhost,).await
    }
}

// ── Internal helpers ─────────────────────────────────────────────────────────

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

        *guard = Some(RabbitMqContainer { container, amqp_port, mgmt_port, },);
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
