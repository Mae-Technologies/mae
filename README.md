For development rules, see [DEVELOPMENT.md](DEVELOPMENT.md)

# mae

Opinionated async Rust framework for building [Mae-Technologies](https://github.com/Mae-Technologies) micro-services.

## Features

- **App scaffolding** — `app::build`, `app::configuration`, `app::run`: opinionated Actix-Web server setup with session middleware, tracing, and Redis
- **Repository layer** — `repo`: typed async CRUD helpers over SQLx/Postgres
- **Middleware** — `middleware`: session extraction, service-to-service auth, request context injection
- **Telemetry** — `telemetry`: structured JSON logging via `tracing-bunyan-formatter`
- **Test utilities** (`test-utils` feature) — TestContainer helpers for Postgres, Neo4j, Redis, RabbitMQ; `must::*` assertion helpers; env helpers

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
mae = "0.1"
```

For test utilities:

```toml
[dev-dependencies]
mae = { version = "0.1", features = ["test-utils"] }
```

## Postgres test helper modes (`test-utils`)

Postgres test helpers support two modes:

- **Container mode** (`MAE_TESTCONTAINERS=1`): starts `ghcr.io/mae-technologies/postgres-mae` via testcontainers (existing behavior).
- **Fallback mode** (default when `MAE_TESTCONTAINERS` is unset/false): connects to an already-running Postgres.

Fallback mode uses safe defaults and allows env overrides:

- `MAE_TEST_PG_HOST` (default `127.0.0.1`)
- `MAE_TEST_PG_PORT` (default `5432`)
- `MAE_TEST_PG_DB` (default `mae_test`)
- `MAE_TEST_PG_USER` (default `app`)
- `MAE_TEST_PG_PASSWORD` (default `secret`)
- `MAE_TEST_PG_MIGRATOR_USER` (default `db_migrator`)
- `MAE_TEST_PG_MIGRATOR_PASSWORD` (default `migrator_secret`)
- `MAE_TEST_PG_SEARCH_PATH` (default `options=-csearch_path%3Dapp`)

`spawn_scoped_schema()` and shared pool setup work in both modes.
For safety, fallback mode refuses database names that do not include `_test`.

## License

MIT
