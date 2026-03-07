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

## License

MIT
