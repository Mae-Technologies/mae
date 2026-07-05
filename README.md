# mae

Opinionated async Rust framework for building [Mae-Technologies](https://github.com/Mae-Technologies) micro-services.

Pair with [`mae_macros`](https://crates.io/crates/mae_macros) for proc-macro helpers (`#[run_app]`, `#[schema]`, `#[mae_test]`). Request/response DTOs live in your service or in a shared models crate — not in `mae` itself.

## Install

```toml
[dependencies]
mae = "0.3"
mae_macros = "0.1"

[dev-dependencies]
mae = { version = "0.3", features = ["test-utils"] }
```

## What you get

| Module | Purpose |
|--------|---------|
| [`app`](src/app/mod.rs) | Config loading, context builder, Actix server runner |
| [`context`](src/context.rs) | `RequestContext<YourAppContext>` — pool, session, custom config per request |
| [`repo`](src/repo/mod.rs) | Typed Postgres repository layer (`WithExecutor`, filters, `DomainStatus`) |
| [`route`](src/route/mod.rs) | `Success` / `ServiceError` / `ServiceResult` HTTP envelope + health routes |
| [`middleware`](src/middleware/mod.rs) | Session and micro-service auth extractors (wired by `#[run_app]`) |
| [`session`](src/session.rs) | Session identity (`user_id`, `sys_client_id`) |
| [`service`](src/service.rs) | `HttpServiceClient` for service-to-service HTTP calls |
| [`crypto`](src/crypto.rs) | AES-256-GCM field encryption for secrets at rest |
| [`totp`](src/totp.rs) | RFC 6238 TOTP generate / verify / otpauth URI |
| [`util`](src/util.rs) | Small helpers (e.g. query-string builder) |
| [`testing`](src/testing/mod.rs) | Integration-test helpers (`test-utils` feature only) |

Internal wiring (`repo::__private__`, container refcount guards, etc.) is not part of the public API — depend only on the modules above.

## Quick start — new service

### 1. Configuration

```rust
use mae::app::build::DeriveContext;
use serde::Deserialize;

#[derive(Clone, Deserialize, DeriveContext)]
pub struct AppContext {
    pub port: u16,
    // service-specific fields from configuration/*.yaml
}
```

Load config in `main.rs`:

```rust
use mae::app::configuration::get_configuration;
use mae::app::run;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = get_configuration().expect("failed to read configuration");
    let app_ctx = mae::app::build::DeriveContext::context(&config.custom);
    run(config, app_ctx).await
}
```

### 2. HTTP routes (`#[run_app]`)

```rust
use actix_web::web;
use mae::app::prelude::*;
use mae_macros::run_app;

#[run_app]
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/api").configure(my_service::route::register));
}
```

`#[run_app]` expands into a full `run()` that attaches Redis sessions, CORS, tracing, health checks, and your routes.

### 3. Domain repository (`#[schema]`)

Define only business fields — audit columns are injected by the macro:

```rust
use mae::repo::macros::schema;
use mae::repo::prelude::*;

#[schema(AppContext, "app")]
pub struct WidgetRepo {
    pub name: String,
    pub kind: String,
}

// Use WithExecutor in usecases:
// WidgetRepo::find().filter(...).fetch_one(&ctx).await?
```

Use `#[schema_root]` for the `sys_client` table (no `sys_client` FK column).

### 4. Controllers — standard response envelope

```rust
use mae::route::response::{ServiceResult, Success};

pub async fn get_widget(ctx: RequestContext<AppContext>) -> ServiceResult<WidgetDto> {
    let row = usecase::get_widget(&ctx, id).await?;
    Success::ok(row.into())
}
```

Clients always receive `{ "data": ... }` on success; errors map to consistent HTTP status codes.

### 5. Usecases — transactions

```rust
use mae::repo::WithExecutor;
use mae::route::response::ServiceError;

pub async fn create_widget(ctx: &RequestContext<AppContext>, input: CreateWidget) -> Result<WidgetRepo, ServiceError> {
    let mut tx = ctx.pg_context.begin().await?;
    let row = WidgetRepo::insert(input.into(), &mut tx).await?;
    tx.commit().await?;
    Ok(row)
}
```

### 6. Service-to-service calls

```rust
use mae::context::ContextAccessor;
use mae::service::{HttpServiceClient, ServiceClientConfig};

let client = HttpServiceClient::new(ServiceClientConfig {
    base_url: ctx.custom().queue_service_url.clone(),
    user_id: ctx.session().user_id,
    micro_service_key: ctx.custom().micro_service_key.clone(),
    micro_service_pass: ctx.custom().micro_service_pass.clone(),
});
let body: QueueStatus = client.get("/internal/status").await?;
```

Downstream services should expose the same `ServiceResult` JSON envelope.

### 7. Encrypted fields + TOTP (optional)

```rust
use mae::crypto::{decrypt_field, encrypt_field};
use mae::totp::{generate_secret, otpauth_uri, verify_code};
```

## Integration testing (`test-utils`)

```rust
use mae::testing::{context::TestContext, must::*};
use mae_macros::mae_test;

#[mae_test(docker, teardown = mae::testing::containers::teardown_all)]
async fn journey_create_widget() -> Result<(), anyhow::Error> {
    let ctx = TestContext::<()>::new().await?;
    // pool + isolated schema; use must_* helpers instead of assert!/unwrap
    Ok(())
}
```

Run docker-gated tests:

```bash
MAE_TESTCONTAINERS=1 cargo test --features test-utils
```

Postgres helpers also support a **fallback mode** (connect to an existing instance) when `MAE_TESTCONTAINERS` is unset. See environment variables in [DEVELOPMENT.md](DEVELOPMENT.md).

## Built-in health endpoints

Registered automatically by `#[run_app]`:

- `GET /health` — process liveness
- `GET /health/pg` — Postgres connectivity
- `GET /health/neo` — Neo4j connectivity

## Related crates

- [`mae_macros`](https://crates.io/crates/mae_macros) — proc macros used throughout the examples above
- [`statbook_models`](https://github.com/Mae-Technologies/statbook_models) — shared API DTOs for Statbook.io services (separate crate; not re-exported by `mae`)

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md) for local setup, smoke tests, and contribution rules.

## License

MIT