# Postgres Table Factory + ACL + Audit (Rust-dev reference)

This directory (`admin_migrations/`) installs a **database-enforced** security model for:
- safe table creation from a JSON spec (no raw SQL injection),
- audit timestamps + immutable columns via triggers,
- least-privilege runtime access via **column-level** GRANTs,
- hard blocks on ad-hoc DDL and privilege changes via **event triggers**.

The important thing to internalize: **your app user does not get DDL or broad UPDATE**, and “what’s writable” is controlled both by **GRANTs** *and* by **triggers**.

---

## Roles and responsibilities (what SQL actually enforces)

### `app_owner` (NOLOGIN)
- Owns the `app` schema objects (tables/functions) and is the owner of `SECURITY DEFINER` functions.
- Because functions are `SECURITY DEFINER`, they run with `app_owner` privileges.

### `app_user` (NOLOGIN)
- This is the “capabilities role” for runtime.
- `apply_table_acl()` grants `app_user`:
  - `SELECT` on the table (all columns)
  - `INSERT` only on an allowlist of columns
  - `UPDATE` only on an allowlist of columns
  - `USAGE, SELECT` on the `id` identity/serial sequence

### `app_migrator` (NOLOGIN)
- Intended membership for your SQLx migrator login.
- **However:** DDL is globally constrained by an event trigger (see **DDL lockdown** below). This is not “normal super-powered migrations”.

### `table_creator` (NOLOGIN)
- Has schema `CREATE` privileges on `app` (granted in `000100_roles.sql`).
- **But** table creation is still blocked by the DDL event trigger unless it happens through `SECURITY DEFINER` functions.

---

## Login roles (created by the bootstrap script)

The bootstrap script creates LOGIN roles (actual DB users) and then grants them memberships into NOLOGIN roles:
- `db_migrator` (LOGIN): runs **app** migrations in `migrations/`
- `app` (LOGIN): application runtime connection
- `table_provisioner` (LOGIN): optional “provision tables at runtime” connection

Memberships granted by the script:
- `GRANT app_user TO app;`
- `GRANT app_migrator TO db_migrator;`
- `GRANT table_creator TO table_provisioner;`
- `GRANT app_user TO db_migrator;` (so the migrator can also do runtime-like actions if needed)
- `GRANT app_user TO table_provisioner;` (so the table_creator can also do runtime-like actions if needed)

---

## DDL and GRANT lockdown (very important)

Two event triggers are installed:

### 1) `trg_block_disallowed_ddl` (`app.block_disallowed_ddl()`)
Blocks (for non-admin effective roles) operations like:
- `CREATE TABLE`, `ALTER TABLE`, `DROP TABLE`, `CREATE SEQUENCE`, etc.

**Exception:** SQLx bookkeeping DDL on `_sqlx_migrations` is allowed for `db_migrator` / `app_migrator` membership.

Result:
- Your normal SQL migrations **cannot** freely create/alter tables in `app` unless the DDL is routed through approved `SECURITY DEFINER` functions.

### 2) `trg_block_disallowed_grants` (`app.block_disallowed_grants()`)
Blocks privilege manipulation (`GRANT`, `REVOKE`, `ALTER DEFAULT PRIVILEGES`) for non-admin effective roles.

Result:
- You cannot “fix permissions in a migration” as the migrator. ACL must be applied via the provided definer function(s).

---

## Audit + immutability model

### Trigger: `app.audit_enforce_timestamps_and_immutables()`
Installed per generated table. Enforces:
- On `INSERT`: sets `updated_at = now()`
- On `UPDATE`:
  - bumps `updated_at = now()`
  - rejects changes to: `id`, `sys_client`, `created_at`, `created_by`
  - requires `updated_by` to change (if it didn’t change, update is rejected)

### Additional trigger: `app.enforce_immutable_columns()`
There is also a separate mechanism using:
- `app.table_column_policies(table_name, immutable_columns)`
- `app.enforce_immutable_columns()` (BEFORE UPDATE trigger)
- `app.upsert_table_column_policy()` helper

This mechanism allows “insert-only columns” to be enforced at the DB level.

**Note:** as currently written, `apply_table_acl()` force-adds these columns to the immutable list:
`id, sys_client, created_at, created_by, status, sys_detail, tags`.

That means **even if GRANTs allow UPDATE of `status/tags/sys_detail`, the trigger can still block changing them**.

---

## Table factory: how to create a table

### Function: `app.create_table_from_spec(jsonb)`
- Validates `table_name` and column identifiers using a strict regex.
- Resolves types using `to_regtype(...)`.
- Only allows scalar JSON defaults (no expressions).
- Creates the table in schema `app`.
- Attaches `audit_enforce_timestamps_and_immutables` trigger.
- Calls `app.apply_table_acl(table, insertable_extras, updatable_extras)`.

**Invoker restriction (important):**
`create_table_from_spec` currently enforces:
- `session_user` must be `'db_migrator'` or `'app_owner'`.

So **`table_provisioner` cannot call it** unless you change that condition.

### JSON spec format

```json
{
  "table_name": "tasks",
  "columns": [
    { "name": "title", "type": "text", "nullable": false },
    { "name": "priority", "type": "int4", "default": 0 },
    { "name": "due_at", "type": "timestamptz" }
  ]
}

### Example invocation

```sql
SELECT app.create_table_from_spec(
  '{
    "table_name":"tasks",
    "columns":[
      {"name":"title","type":"text","nullable":false},
      {"name":"priority","type":"int4","default":0},
      {"name":"due_at","type":"timestamptz"}
    ]
  }'::jsonb
);
```

---

## ACLs: default vs “modified ACL”

### Function: `app.apply_table_acl(table_name, insertable_cols, updatable_cols)`

It:

* sets table owner to `app_owner`,
* revokes all existing privileges from `PUBLIC` and `app_user`,
* grants `SELECT` on table,
* grants column-level `INSERT` and `UPDATE` to `app_user`,
* grants `USAGE, SELECT` on the `id` sequence,
* updates the immutable-column policy table.

### Default allowlists

`apply_table_acl()` always appends these defaults:

Insert defaults:

* `sys_client`, `status`, `comment`, `tags`, `sys_detail`, `created_by`, `updated_by`

Update defaults:

* `status`, `comment`, `tags`, `sys_detail`, `updated_by`

It also appends “extras” you pass for spec columns.

### Example: make a column insert-only

Goal: `priority` insertable but not updatable.

```sql
-- Insert extras: title, priority
-- Update extras:  title
SELECT app.apply_table_acl(
  'tasks',
  ARRAY['title','priority'],
  ARRAY['title']
);
```

This causes:

* `priority` to be GRANTed for INSERT but not UPDATE
* `priority` to be included in the immutable policy (insertable \ updatable)

---

## What app runtime is NOT allowed to do

If your app connects as `app` (member of `app_user`):

* cannot `CREATE TABLE`, `ALTER TABLE`, `DROP TABLE`, `CREATE INDEX` (blocked by DDL event trigger),
* cannot `GRANT`/`REVOKE` anything (blocked by GRANT event trigger),
* cannot update protected audit fields (`id`, `sys_client`, `created_at`, `created_by`) due to audit trigger,
* cannot “forget” to update `updated_by` on UPDATE (audit trigger will reject the UPDATE),
* cannot INSERT/UPDATE columns not explicitly allowlisted by `apply_table_acl()`.

---

## What the migrator is NOT allowed to do

If your migrator connects as `db_migrator` (member of `app_migrator` and `app_user`):

* cannot do arbitrary DDL in `app` (blocked by DDL event trigger),
* can only do SQLx bookkeeping DDL for `_sqlx_migrations`,
* cannot `GRANT`/`REVOKE` privileges (blocked by GRANT event trigger),
* should rely on `create_table_from_spec` and `apply_table_acl` (or other approved definer functions) rather than raw DDL.

---

## Local bootstrap: how to run the script

Script file: `admin_migrations/sqlx_premigration.sh` (must be executable).

### Requirements

* Docker
* sqlx-cli (~0.8), postgres feature, rustls recommended:

```bash
cargo install --version='~0.8' sqlx-cli --no-default-features --features rustls,postgres
```

### Run (from repo root)

This script expects `.env` in the current working directory.

```bash
chmod +x admin_migrations/sqlx_premigration.sh
./admin_migrations/sqlx_premigration.sh
```

Behavior:

* starts a local Postgres docker container (unless `CONTAINER` is set),
* creates the database,
* creates LOGIN roles,
* runs `admin_migrations/` as superuser,
* grants role memberships to the LOGIN roles,
* optionally runs `migrations/` as `db_migrator` when `RUN_APP_MIGRATIONS=1`.

---

## `.env` (required by the script)

The script **does not default** missing values; it hard-requires them.

Create/update `.env` at repo root:

* `DB_HOST`, `DB_PORT`, `APP_DB_NAME`
* `SUPERUSER`, `SUPERUSER_PWD`
* `MIGRATOR_USER`, `MIGRATOR_PWD`
* `APP_USER`, `APP_USER_PWD`
* `TABLE_PROVISIONER_USER`, `TABLE_PROVISIONER_PWD`
* `ADMIN_MIGRATIONS_PATH`, `APP_MIGRATIONS_PATH`
* `SEARCH_PATH` (URL-encoded search_path option)

Minimal template:

```env
RUN_APP_MIGRATIONS=1

ADMIN_MIGRATIONS_PATH=admin_migrations
APP_MIGRATIONS_PATH=migrations

DB_HOST=127.0.0.1
DB_PORT=2345
APP_DB_NAME=mae_test

SUPERUSER=postgres
SUPERUSER_PWD=password

MIGRATOR_USER=db_migrator
MIGRATOR_PWD=migrator_secret

APP_USER=app
APP_USER_PWD=secret

TABLE_PROVISIONER_USER=table_provisioner
TABLE_PROVISIONER_PWD=provisioner_secret

# example: search_path=app
SEARCH_PATH=options=-csearch_path%3Dapp
```

Notes:

* The script constructs database URLs internally from these values.
* If you also define `DATABASE_URL` / `APP_DATABASE_URL` / etc, they are not used by the script unless you manually call sqlx outside the script.

---

## Rust/SQLx usage patterns

* **Migrations that define new domain tables** should generally call:

  * `SELECT app.create_table_from_spec(...)`
  * optionally `SELECT app.apply_table_acl(...)` again to harden

* Application queries should:

  * always set `created_by` on INSERT,
  * always change `updated_by` on UPDATE,
  * never try to update `sys_client/created_by/created_at/id`.

---

## Quick checklist

When something fails:

* `permission denied for sequence ..._id_seq`
  → `apply_table_acl()` did not run for that table (or you ran it before the identity sequence existed).

* `DDL "CREATE TABLE": not allowed ... Use create_table_from_spec/apply_table_acl.`
  → expected; use the factory function.

* `updated_by must be updated`
  → your UPDATE statement didn’t modify `updated_by`.

* `column "X" is immutable on table "Y"`
  → immutable policy trigger fired; adjust allowlists or policy logic.

### 2) `.env` header comment is misleading

Your `.env` currently says missing variables “fall back to defaults defined in init_db.sh”, but the script hard-requires variables and exits if they’re missing. Update the `.env` comment to reflect: **all required vars must be set**.

### 3) Table provisioning mismatch (important)

* **Option B:** don’t use runtime provisioning; keep table creation in migrations.

---

## One targeted question

Do you want runtime provisioning (`table_provisioner`) to be supported as-is (by changing `create_table_from_spec` invoker checks), or do you want table creation to be migration-only (keep current restriction)?
