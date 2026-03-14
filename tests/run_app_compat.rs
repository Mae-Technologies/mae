//! Compile-time compat check: `#[run_app]` macro vs `Run` trait.
//!
//! This file does **not** run any runtime logic.  Its sole purpose is to be
//! **compiled**: if `#[run_app]` generates a `run` signature that no longer
//! matches `Run::run`, this file will produce a compile error and CI will
//! surface the divergence immediately.
//!
//! History: mae 0.3.0 added `graph_pool: neo4rs::Graph` to `Run::run`.
//! `mae_macros::run_app` was not updated at the same time; the mismatch was
//! caught late.  This test prevents that from happening again.
//!
//! Run with:
//!   cargo test --features integration-testing --test run_app_compat

#![cfg(feature = "integration-testing")]

use mae::app;
use mae::app::prelude::*;

// ── Minimal App implementation ────────────────────────────────────────────────

struct CompatCheck {
    port: u16,
    server: Server
}

impl App for CompatCheck {
    fn new(port: u16, server: Server) -> Self {
        Self { port, server }
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn server(self) -> Server {
        self.server
    }
}

// ── Run implementation via #[run_app] ─────────────────────────────────────────
//
// The macro rewrites this entire function, generating a hardcoded signature.
// If that generated signature does not satisfy the `Run` trait (e.g. a new
// parameter was added to the trait but the macro was not updated), this impl
// block will fail to compile.
//
// NOTE: The macro ignores the written parameter list and return type below —
// only the first *statement* in the body is used (it becomes the final
// method-chain segment on the Actix-Web `App` builder).

impl Run for CompatCheck {
    #[run_app]
    fn run<Context: Clone + Send + 'static>(
        _listener: TcpListener,
        _db_pool: PgPool,
        _graph_pool: neo4rs::Graph,
        _base_url: String,
        _hmac_secret: SecretString,
        _redis_uri: SecretString,
        _custom_context: Context
    ) -> impl std::future::Future<Output = Result<Server, anyhow::Error>> + Send {
        // The macro picks up only the first statement and splices it as the
        // last builder chain call (.first_statement).  A no-op scope is the
        // lightest possible valid actix-web configure call.
        service(web::scope(""))
    }
}

// ── Placeholder test so `cargo test` has something to report ─────────────────

/// This test has no assertions.  The real regression check is whether this
/// file *compiles* at all.
#[test]
fn run_app_macro_matches_run_trait() {}
