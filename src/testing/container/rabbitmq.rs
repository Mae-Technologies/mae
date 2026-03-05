//! RabbitMQ container singleton + per-test vhost isolation.
//!
//! # Status — issue #40
//! Container spinup is implemented in issue #40.
//!
//! # Isolation strategy
//! RabbitMQ virtual hosts (vhosts) provide complete namespace isolation.
//! Each test scope owns a unique vhost (`/test_<uuid>`).  On teardown the
//! vhost is deleted via the management HTTP API, removing all exchanges,
//! queues, and bindings within it.

use anyhow::Result;
use tokio::sync::{Mutex, OnceCell};
use uuid::Uuid;

use super::MaeContainer;

// ── Singleton placeholder ─────────────────────────────────────────────────────

pub struct Inner {
    // Populated by #40:
    //   pub container: ContainerAsync<RabbitMq>,
    //   pub id: String,
    //   pub mgmt_port: u16,
    #[allow(dead_code)]
    pub amqp_port: u16,
}

static SINGLETON: OnceCell<Mutex<Option<Inner,>,>,> = OnceCell::const_new();

// ── Public isolation scope ────────────────────────────────────────────────────

/// Per-test isolation scope for RabbitMQ.
pub struct RabbitMqScope {
    /// Virtual host name assigned to this scope (e.g. `/test_<uuid>`).
    pub vhost: String,
}

impl RabbitMqScope {
    /// Delete this vhost via the RabbitMQ management API.
    ///
    /// This removes all exchanges, queues, and bindings within the vhost.
    ///
    /// # Note — implemented in #40.
    pub async fn delete_vhost(&self,) -> Result<(),> {
        // TODO (#40): DELETE /api/vhosts/<vhost> via management HTTP API.
        Ok((),)
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

/// Returns the RabbitMQ singleton (stub until #40).
pub async fn rabbitmq_singleton() -> &'static Mutex<Option<Inner,>,> {
    SINGLETON
        .get_or_init(|| async {
            // TODO (#40): start RabbitMQ testcontainer when MAE_TESTCONTAINERS=1.
            Mutex::new(None,)
        },)
        .await
}

/// Create a fresh per-test vhost scope.
///
/// # Note — implemented in #40.
pub async fn spawn_scoped_vhost() -> Result<RabbitMqScope,> {
    let guard = rabbitmq_singleton().await.lock().await;
    let _inner = guard.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "RabbitMQ container not running — set MAE_TESTCONTAINERS=1 (spinup lands in #40)"
        )
    },)?;

    // TODO (#40): POST /api/vhosts/<vhost> via management HTTP API.
    let vhost = format!("/test_{}", Uuid::new_v4().to_string().replace('-', ""));
    Ok(RabbitMqScope { vhost, },)
}

/// Stop the RabbitMQ container and reset the singleton.
pub async fn teardown() {
    if let Some(m,) = SINGLETON.get() {
        let mut guard = m.lock().await;
        let _ = guard.take();
    }
}
