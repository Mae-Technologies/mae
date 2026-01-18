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

is_debug() {
  [[ "${DEBUG:-}" == "1" ]]
}

# stdout is a terminal
is_tty() {
  [[ -t 1 ]]
}

# -----------------------------------------------------------------------------
# Logging helpers (emoji + two spaces, TTY-only)
# -----------------------------------------------------------------------------
c_reset="\033[0m"
c_blue="\033[34m"
c_green="\033[32m"
c_yellow="\033[33m"
c_red="\033[31m"

log_info() {
  is_tty || return 0
  is_debug || return 0
  echo -e "${c_blue}🧩  $*${c_reset}"
}

log_ok() {
  is_tty || return 0
  echo -e "${c_green}✅  $*${c_reset}"
}

log_warn() {
  is_tty || return 0
  is_debug || return 0
  echo -e "${c_yellow}⚠️  $*${c_reset}"
}

log_err() {
  # stderr TTY check is more correct for errors
  [[ -t 2 ]] || return 0
  echo -e "${c_red}❌  $*${c_reset}" >&2
}

# -----------------------------------------------------------------------------
# Tooling checks
# -----------------------------------------------------------------------------
if ! [ -x "$(command -v sqlx)" ]; then
  log_err "sqlx is not installed"
  echo >&2 "Install:"
  echo >&2 "  cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,postgres"
  exit 1
fi

if ! [ -x "$(command -v docker)" ]; then
  log_err "docker is not installed"
  exit 1
fi

# -----------------------------------------------------------------------------
# Load .env if present
# -----------------------------------------------------------------------------
if [[ -z "${NO_DOT_ENV:-}" && -f ".env" ]]; then
  log_info "Loading .env"
  set -a
  # shellcheck disable=SC1091
  source ".env"
  set +a
  log_ok "Loaded .env"
elif [[ -n "${NO_DOT_ENV:-}" ]]; then
  log_info "NO_DOT_ENV set; skipping .env"
else
  log_warn "No .env found"
fi

# -----------------------------------------------------------------------------
# Require variables (do NOT default)
# -----------------------------------------------------------------------------
require_var() {
  local name="$1"
  # ${!name+x} checks "is set", even if empty; then also reject empty explicitly
  if [[ -z "${!name+x}" ]]; then
    log_err "Required env var not set: ${name}"
    exit 1
  fi
  if [[ -z "${!name}" ]]; then
    log_err "Required env var is empty: ${name}"
    exit 1
  fi
}

# Required core config
require_var DB_PORT
require_var DB_HOST
require_var APP_DB_NAME

require_var SUPERUSER
require_var SUPERUSER_PWD

require_var MIGRATOR_USER
require_var MIGRATOR_PWD

require_var APP_USER
require_var APP_USER_PWD

require_var TABLE_PROVISIONER_USER
require_var TABLE_PROVISIONER_PWD

require_var ADMIN_MIGRATIONS_PATH
require_var APP_MIGRATIONS_PATH

# -----------------------------------------------------------------------------
# Build DB URLs ONLY if component vars exist; also ensure DB_HOST is used.
# If a *_DATABASE_URL is provided, keep it; otherwise build it.
# -----------------------------------------------------------------------------
if [[ -z "${SUPER_DATABASE_URL:-}" ]]; then
  SUPER_DATABASE_URL="postgres://${SUPERUSER}:${SUPERUSER_PWD}@${DB_HOST}:${DB_PORT}/${APP_DB_NAME}"
fi

if [[ -z "${MIGRATOR_DATABASE_URL:-}" ]]; then
  MIGRATOR_DATABASE_URL="postgres://${MIGRATOR_USER}:${MIGRATOR_PWD}@${DB_HOST}:${DB_PORT}/${APP_DB_NAME}"
fi

if [[ -z "${APP_DATABASE_URL:-}" ]]; then
  APP_DATABASE_URL="postgres://${APP_USER}:${APP_USER_PWD}@${DB_HOST}:${DB_PORT}/${APP_DB_NAME}"
fi

if [[ -z "${TABLE_CREATOR_DATABASE_URL:-}" ]]; then
  TABLE_CREATOR_DATABASE_URL="postgres://${TABLE_PROVISIONER_USER}:${TABLE_PROVISIONER_PWD}@${DB_HOST}:${DB_PORT}/${APP_DB_NAME}"
fi

log_ok "Environment validated"

# -----------------------------------------------------------------------------
# Docker bootstrap
# -----------------------------------------------------------------------------

if [[ -z "${CONTAINER}" ]]; then
  log_info "Starting Postgres container on port ${DB_PORT}"

  RUNNING_POSTGRES_CONTAINER=$(docker ps --filter 'name=postgres' --filter "publish=${DB_PORT}" --format '{{.ID}}')
  if [[ -n "${RUNNING_POSTGRES_CONTAINER}" ]]; then
    log_err "A postgres container is already running on port ${DB_PORT}"
    echo >&2 "Kill it with:"
    echo >&2 "  docker kill ${RUNNING_POSTGRES_CONTAINER}"
    exit 1
  fi

  CONTAINER="mae_service_pg_$(uuidgen)"

  docker run \
    --env POSTGRES_USER="${SUPERUSER}" \
    --env POSTGRES_PASSWORD="${SUPERUSER_PWD}" \
    --health-cmd="pg_isready -U ${SUPERUSER} || exit 1" \
    --health-interval=1s \
    --health-timeout=5s \
    --health-retries=10 \
    --publish "${DB_PORT}":5432 \
    --detach \
    --name "${CONTAINER}" \
    postgres -N 1000 >/dev/null

  until [[ "$(docker inspect -f "{{.State.Health.Status}}" "${CONTAINER}")" == "healthy" ]]; do
    log_warn "Postgres is still unavailable - sleeping"
    sleep 1
  done

  log_ok "Postgres container is healthy"
else
  log_warn "CONTAINER set; assuming Postgres is reachable on ${DB_HOST}:${DB_PORT}"
fi

# -----------------------------------------------------------------------------
# Quiet Postgres helpers (stdout suppressed; errors still shown)
# -----------------------------------------------------------------------------
psql_super_db() {
  local db="$1"
  local sql="$2"
  if [[ -n "${CONTAINER}" ]]; then
    docker exec -i "${CONTAINER}" \
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
log_ok "Database ensured: ${SUPER_DATABASE_URL}"

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

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${MIGRATOR_USER}') THEN
    CREATE ROLE ${MIGRATOR_USER} LOGIN PASSWORD '${MIGRATOR_PWD}';
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = '${TABLE_PROVISIONER_USER}') THEN
    CREATE ROLE ${TABLE_PROVISIONER_USER} LOGIN PASSWORD '${TABLE_PROVISIONER_PWD}';
  END IF;
END
\$\$;
"
log_ok "LOGIN roles ensured"

# -----------------------------------------------------------------------------
# 4) Run admin migrations as SUPERUSER against admin_migrations/
# -----------------------------------------------------------------------------
# This is where 01-05 live now (roles/functions/lockdowns).
log_info "Running admin migrations (superuser) from ./${ADMIN_MIGRATIONS_PATH}/"
if [[ "${DEBUG:-}" == "1" ]]; then
  sqlx migrate run --no-dotenv --database-url "${SUPER_DATABASE_URL}" --source "${ADMIN_MIGRATIONS_PATH}"
else
  sqlx migrate run --no-dotenv --database-url "${SUPER_DATABASE_URL}" --source "${ADMIN_MIGRATIONS_PATH}" >/dev/null
fi
log_ok "Admin migrations applied"

# -----------------------------------------------------------------------------
# 5) Grant runtime memberships
# -----------------------------------------------------------------------------
# These roles are expected to be created by admin_migrations:
#   - app_user
#   - table_creator
#   - migrator_user
log_info "Granting runtime role memberships (superuser)"
psql_super_db "${APP_DB_NAME}" "GRANT app_user TO ${APP_USER};"
psql_super_db "${APP_DB_NAME}" "GRANT table_creator TO ${TABLE_PROVISIONER_USER};"
psql_super_db "${APP_DB_NAME}" "GRANT app_migrator TO ${MIGRATOR_USER};"
# NOTE: we also want the migrator to have the same priviledges as the app_user (IE - USAGE)
psql_super_db "${APP_DB_NAME}" "GRANT app_user TO ${MIGRATOR_USER};"
log_ok "Runtime memberships granted"

# -----------------------------------------------------------------------------
# 6) Run normal app migrations as MIGRATOR_USER against migrations/
# -----------------------------------------------------------------------------
log_info "Running app migrations (db_migrator) from ./${APP_MIGRATIONS_PATH}/"
if [[ "${DEBUG:-}" == "1" ]]; then
  sqlx migrate run --no-dotenv --database-url "${MIGRATOR_DATABASE_URL}" --source "${APP_MIGRATIONS_PATH}"
else
  sqlx migrate run --no-dotenv --database-url "${MIGRATOR_DATABASE_URL}" --source "${APP_MIGRATIONS_PATH}" >/dev/null
fi
log_ok "App migrations applied"

log_ok "Done"
# log_info "Runtime connection string (app): ${APP_DATABASE_URL}"
# log_info "Provisioning connection string (optional): ${TABLE_CREATOR_DATABASE_URL}"
# log_info "Migrator connection string (app migrations): ${MIGRATOR_DATABASE_URL}"
# log_info "Admin connection string (admin migrations): ${SUPER_DATABASE_URL}"
