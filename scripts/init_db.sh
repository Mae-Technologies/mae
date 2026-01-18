#!/usr/bin/bash
# init_db.sh
#
# Starts local Postgres (Docker), creates LOGIN roles used by SQLx + the app,
# runs migrations, and then grants membership to NOLOGIN roles.
#
# Requirements implemented:
#!/usr/bin/bash
# init_db.sh
#
# Production/dev/CI bootstrap with a strict separation:
#   - admin_migrations/: executed as SUPERUSER (high-trust). Contains 01-05 (roles/functions/lockdowns).
#   - migrations/: executed as MIGRATOR_USER (low-trust). Contains normal app schema migrations.
#
# Key security goals:
#   - MIGRATOR_USER has minimal, typical production privileges
#   - All role creation and sensitive privilege/ownership operations occur only under SUPERUSER
#
# SQLx usage:
#   - Admin migrations:
#       sqlx migrate run --database-url <superuser-url> --source admin_migrations
#   - App migrations:
#       sqlx migrate run --database-url <migrator-url> --source migrations
#
# Output behavior:
#   - Postgres (psql) output suppressed unless error
#   - sqlx output NOT suppressed
#   - Colored, emoji-prefixed stage logs (emoji + two spaces)

set -eo pipefail

# -----------------------------------------------------------------------------
# Logging helpers (emoji + two spaces)
# -----------------------------------------------------------------------------
c_reset="\033[0m"
c_blue="\033[34m"
c_green="\033[32m"
c_yellow="\033[33m"
c_red="\033[31m"

log_info() { echo -e "${c_blue}🧩  $*${c_reset}"; }
log_ok() { echo -e "${c_green}✅  $*${c_reset}"; }
log_warn() { echo -e "${c_yellow}⚠️  $*${c_reset}"; }
log_error() { echo -e "${c_red}❌  $*${c_reset}"; }

# -----------------------------------------------------------------------------
# Load .env if present
# -----------------------------------------------------------------------------
if [[ -f ".env" ]]; then
  log_info "Loading .env"
  set -a
  # shellcheck disable=SC1091
  source ".env"
  set +a
  log_ok "Loaded .env"
else
  log_warn "No .env found; using defaults"
fi

# -----------------------------------------------------------------------------
# Tooling checks
# -----------------------------------------------------------------------------
if ! [ -x "$(command -v sqlx)" ]; then
  log_error "sqlx is not installed"
  echo >&2 "Install:"
  echo >&2 "  cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,postgres"
  exit 1
fi

if ! [ -x "$(command -v docker)" ]; then
  log_error "docker is not installed"
  exit 1
fi

# -----------------------------------------------------------------------------
# Defaults (used only when not set by .env / environment)
# -----------------------------------------------------------------------------
DB_PORT="${DB_PORT:-2345}"
APP_DB_NAME="${APP_DB_NAME:-test_mae}"

SUPERUSER="${SUPERUSER:-postgres}"
SUPERUSER_PWD="${SUPERUSER_PWD:-password}"

MIGRATOR_USER="${MIGRATOR_USER:-db_migrator}"
MIGRATOR_PWD="${MIGRATOR_PWD:-migrator_secret}"

APP_USER="${APP_USER:-app}"
APP_USER_PWD="${APP_USER_PWD:-secret}"

TABLE_PROVISIONER_USER="${TABLE_PROVISIONER_USER:-table_provisioner}"
TABLE_PROVISIONER_PWD="${TABLE_PROVISIONER_PWD:-provisioner_secret}"

# Build DB URLs if not already provided.
# These are used explicitly via --database-url, not implicitly via DATABASE_URL.
SUPER_DATABASE_URL="${SUPER_DATABASE_URL:-postgres://${SUPERUSER}:${SUPERUSER_PWD}@127.0.0.1:${DB_PORT}/${APP_DB_NAME}}"
MIGRATOR_DATABASE_URL="${MIGRATOR_DATABASE_URL:-postgres://${MIGRATOR_USER}:${MIGRATOR_PWD}@127.0.0.1:${DB_PORT}/${APP_DB_NAME}}"
APP_DATABASE_URL="${APP_DATABASE_URL:-postgres://${APP_USER}:${APP_USER_PWD}@127.0.0.1:${DB_PORT}/${APP_DB_NAME}}"
TABLE_CREATOR_DATABASE_URL="${TABLE_CREATOR_DATABASE_URL:-postgres://${TABLE_PROVISIONER_USER}:${TABLE_PROVISIONER_PWD}@127.0.0.1:${DB_PORT}/${APP_DB_NAME}}"

ADMIN_MIGRATIONS_PATH="${ADMIN_MIGRATIONS_PATH:-admin_migrations}"
APP_MIGRATIONS_PATH="${APP_MIGRATIONS_PATH:-migrations}"

# -----------------------------------------------------------------------------
# Docker bootstrap
# -----------------------------------------------------------------------------
CONTAINER_NAME=""

if [[ -z "${SKIP_DOCKER}" ]]; then
  log_info "Starting Postgres container on port ${DB_PORT}"

  RUNNING_POSTGRES_CONTAINER=$(docker ps --filter 'name=postgres' --filter "publish=${DB_PORT}" --format '{{.ID}}')
  if [[ -n "${RUNNING_POSTGRES_CONTAINER}" ]]; then
    log_error "A postgres container is already running on port ${DB_PORT}"
    echo >&2 "Kill it with:"
    echo >&2 "  docker kill ${RUNNING_POSTGRES_CONTAINER}"
    exit 1
  fi

  CONTAINER_NAME="mae_service_pg_$(uuidgen)"

  docker run \
    --env POSTGRES_USER="${SUPERUSER}" \
    --env POSTGRES_PASSWORD="${SUPERUSER_PWD}" \
    --health-cmd="pg_isready -U ${SUPERUSER} || exit 1" \
    --health-interval=1s \
    --health-timeout=5s \
    --health-retries=10 \
    --publish "${DB_PORT}":5432 \
    --detach \
    --name "${CONTAINER_NAME}" \
    postgres -N 1000 >/dev/null

  until [[ "$(docker inspect -f "{{.State.Health.Status}}" "${CONTAINER_NAME}")" == "healthy" ]]; do
    log_warn "Postgres is still unavailable - sleeping"
    sleep 1
  done

  log_ok "Postgres container is healthy"
else
  log_warn "SKIP_DOCKER set; assuming Postgres is reachable on 127.0.0.1:${DB_PORT}"
fi

# -----------------------------------------------------------------------------
# Quiet Postgres helpers (stdout suppressed; errors still shown)
# -----------------------------------------------------------------------------
psql_super() {
  local sql="$1"
  if [[ -n "${CONTAINER_NAME}" ]]; then
    docker exec -i "${CONTAINER_NAME}" \
      psql -U "${SUPERUSER}" -v ON_ERROR_STOP=1 -q -c "${sql}" \
      1>/dev/null
  else
    PGPASSWORD="${SUPERUSER_PWD}" psql -h 127.0.0.1 -p "${DB_PORT}" -U "${SUPERUSER}" \
      -v ON_ERROR_STOP=1 -q -c "${sql}" \
      1>/dev/null
  fi
}

psql_super_db() {
  local db="$1"
  local sql="$2"
  if [[ -n "${CONTAINER_NAME}" ]]; then
    docker exec -i "${CONTAINER_NAME}" \
      psql -U "${SUPERUSER}" -d "${db}" -v ON_ERROR_STOP=1 -q -c "${sql}" \
      1>/dev/null
  else
    PGPASSWORD="${SUPERUSER_PWD}" psql -h 127.0.0.1 -p "${DB_PORT}" -U "${SUPERUSER}" \
      -d "${db}" -v ON_ERROR_STOP=1 -q -c "${sql}" \
      1>/dev/null
  fi
}

# -----------------------------------------------------------------------------
# 1) Ensure DB exists (as SUPERUSER)
# -----------------------------------------------------------------------------
log_info "Ensuring database exists via sqlx (superuser)"
sqlx database create --database-url "${SUPER_DATABASE_URL}"
log_ok "Database ensured"

# -----------------------------------------------------------------------------
# 2) Create LOGIN roles (as SUPERUSER) - these are actual DB users
# -----------------------------------------------------------------------------
log_info "Ensuring LOGIN roles exist (superuser)"
psql_super_db "${APP_DB_NAME}" "
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${MIGRATOR_USER}') THEN
    CREATE ROLE ${MIGRATOR_USER} LOGIN PASSWORD '${MIGRATOR_PWD}';
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${APP_USER}') THEN
    CREATE ROLE ${APP_USER} LOGIN PASSWORD '${APP_USER_PWD}';
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${TABLE_PROVISIONER_USER}') THEN
    CREATE ROLE ${TABLE_PROVISIONER_USER} LOGIN PASSWORD '${TABLE_PROVISIONER_PWD}';
  END IF;
END
\$\$;
"
log_ok "LOGIN roles ensured"

# -----------------------------------------------------------------------------
# Ensure NOLOGIN roles exist (admin migrations reference these roles)
# -----------------------------------------------------------------------------
log_info "Ensuring NOLOGIN roles exist (superuser)"

psql_super_db "${APP_DB_NAME}" "
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_owner') THEN
    CREATE ROLE app_owner NOLOGIN;
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_user') THEN
    CREATE ROLE app_user NOLOGIN;
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'table_creator') THEN
    CREATE ROLE table_creator NOLOGIN;
  END IF;
END
\$\$;
"

log_ok "NOLOGIN roles ensured"

# -----------------------------------------------------------------------------
# 3.1) locking public down, working inside app schema
# -----------------------------------------------------------------------------
log_info "locking public schema, creating app schema..."

psql_super_db "$APP_DB_NAME" "
DO \$\$
BEGIN
  -- Run in admin_migrations (superuser/admin).
  CREATE SCHEMA IF NOT EXISTS app AUTHORIZATION app_owner;
  
  -- Lock down public: stop using it for app objects.
  REVOKE CREATE ON SCHEMA public FROM PUBLIC;
  REVOKE CREATE ON SCHEMA public FROM db_migrator;
  
  -- Allow migrator to create objects only in app schema.
  GRANT USAGE, CREATE ON SCHEMA app TO db_migrator;
  
  -- Allow app_owner full use in app (already owns schema, but be explicit).
  GRANT USAGE, CREATE ON SCHEMA app TO app_owner;
  
  -- Runtime only needs USAGE (tables are accessed via table grants).
  GRANT USAGE ON SCHEMA app TO app_user;
END
\$\$"
log_info "public schema locked, app schema created"

# -----------------------------------------------------------------------------
# 3.2) locking search_paths down
# -----------------------------------------------------------------------------
log_info "locking search_paths down"

psql_super_db "$APP_DB_NAME" "
ALTER ROLE ${MIGRATOR_USER} SET search_path = app, public;
ALTER ROLE app_owner SET search_path = app, public;
ALTER ROLE app_user SET search_path = app;
ALTER ROLE table_creator SET search_path = app, public;
ALTER ROLE ${APP_USER} SET search_path = app;
ALTER ROLE ${TABLE_PROVISIONER_USER} SET search_path = app;
"
log_ok "search_path locked down for roles"

# -----------------------------------------------------------------------------
# 4) Run admin migrations as SUPERUSER against admin_migrations/
# -----------------------------------------------------------------------------
# This is where 01-05 live now (roles/functions/lockdowns).
log_info "Running admin migrations (superuser) from ${ADMIN_MIGRATIONS_PATH}"
sqlx migrate run --no-dotenv --database-url "${SUPER_DATABASE_URL}" --source "${ADMIN_MIGRATIONS_PATH}"
log_ok "Admin migrations applied"

# -----------------------------------------------------------------------------
# 4.1) Allow db_migrator to read/write SQLx migration bookkeeping
# -----------------------------------------------------------------------------
log_info "Granting db_migrator access to SQLx bookkeeping table (_sqlx_migrations)"

psql_super_db "${APP_DB_NAME}" "
-- The table is created/owned by the admin migration run (superuser).
-- The migrator must be able to SELECT/INSERT/UPDATE it to record applied migrations.
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public._sqlx_migrations TO ${MIGRATOR_USER};
"

log_ok "db_migrator can now write _sqlx_migrations"

# -----------------------------------------------------------------------------
# 5) Run normal app migrations as MIGRATOR_USER against migrations/
# -----------------------------------------------------------------------------
log_info "Running app migrations (db_migrator) from ${APP_MIGRATIONS_PATH}"
sqlx migrate run --no-dotenv --database-url "${MIGRATOR_DATABASE_URL}" --source "${APP_MIGRATIONS_PATH}" --ignore-missing
log_ok "App migrations applied"

# -----------------------------------------------------------------------------
# 6) Grant runtime memberships (superuser)
# -----------------------------------------------------------------------------
# These roles are expected to be created by admin_migrations:
#   - app_user
#   - table_creator
log_info "Granting runtime role memberships (superuser)"
psql_super_db "${APP_DB_NAME}" "GRANT app_user TO ${APP_USER};"
psql_super_db "${APP_DB_NAME}" "GRANT table_creator TO ${TABLE_PROVISIONER_USER};"
log_ok "Runtime memberships granted"

log_ok "Done"
log_info "Runtime connection string (app): ${APP_DATABASE_URL}"
log_info "Provisioning connection string (optional): ${TABLE_CREATOR_DATABASE_URL}"
log_info "Migrator connection string (app migrations): ${MIGRATOR_DATABASE_URL}"
log_info "Admin connection string (admin migrations): ${SUPER_DATABASE_URL}"
