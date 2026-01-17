-- v2: Create a table from a validated JSONB spec (no raw SQL fragments).
-- SECURITY PROPERTIES:
--   - Identifiers (table/column names) are validated and quoted via %I
--   - Types are resolved by the server using to_regtype(), not string-concatenated
--   - Defaults are limited to JSON scalar literals and rendered via quote_literal()
--   - No support for arbitrary expressions (intentionally), to eliminate injection risk
--
-- EXPECTED JSON SHAPE:
-- {
--   "table_name": "my_table",
--   "columns": [
--     { "name": "title", "type": "text", "nullable": false },
--     { "name": "priority", "type": "int4", "nullable": false, "default": 0 },
--     { "name": "is_done", "type": "bool", "nullable": false, "default": false, "unique": false }
--   ]
-- }
CREATE OR REPLACE FUNCTION public.create_table_from_spec(p_spec jsonb)
RETURNS void
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  v_table_name text;
  v_cols jsonb;

  v_sql text;

  -- Per-column fields
  c jsonb;
  c_name text;
  c_type_text text;
  c_type regtype;
  c_nullable boolean;
  c_unique boolean;
  c_has_default boolean;
  c_default jsonb;

  -- Helpers
  col_defs text := '';
  sep text := '';
  v_kind text;
BEGIN
  ---------------------------------------------------------------------------
  -- 1) Validate presence and type of required keys
  ---------------------------------------------------------------------------
  IF p_spec IS NULL THEN
    RAISE EXCEPTION 'spec must not be null';
  END IF;

  v_table_name := p_spec->>'table_name';
  IF v_table_name IS NULL OR length(v_table_name) = 0 THEN
    RAISE EXCEPTION 'spec.table_name is required';
  END IF;

  -- Basic hardening: allow only simple identifiers for table name.
  -- (We still use %I quoting; this just prevents surprising names.)
  IF v_table_name !~ '^[a-zA-Z_][a-zA-Z0-9_]*$' THEN
    RAISE EXCEPTION 'invalid table_name: %', v_table_name;
  END IF;

  v_cols := p_spec->'columns';
  IF v_cols IS NULL OR jsonb_typeof(v_cols) <> 'array' THEN
    RAISE EXCEPTION 'spec.columns must be an array';
  END IF;

  ---------------------------------------------------------------------------
  -- 2) Build safe column definitions from JSON spec
  ---------------------------------------------------------------------------
  FOR c IN
    SELECT value FROM jsonb_array_elements(v_cols) AS t(value)
  LOOP
    IF jsonb_typeof(c) <> 'object' THEN
      RAISE EXCEPTION 'each item in spec.columns must be an object';
    END IF;

    c_name := c->>'name';
    c_type_text := c->>'type';

    IF c_name IS NULL OR c_type_text IS NULL THEN
      RAISE EXCEPTION 'each column requires "name" and "type"';
    END IF;

    -- Validate column name shape (defense-in-depth; still quoted later).
    IF c_name !~ '^[a-zA-Z_][a-zA-Z0-9_]*$' THEN
      RAISE EXCEPTION 'invalid column name: %', c_name;
    END IF;

    -- Resolve type via server parser. This prevents injecting tokens into the DDL.
    -- Examples accepted: 'text', 'int4', 'uuid', 'timestamptz', 'public.my_enum'
    c_type := to_regtype(c_type_text);
    IF c_type IS NULL THEN
      RAISE EXCEPTION 'unknown or invalid type: %', c_type_text;
    END IF;

    -- Optional flags
    c_nullable := COALESCE((c->>'nullable')::boolean, true);
    c_unique   := COALESCE((c->>'unique')::boolean, false);

    c_has_default := c ? 'default';
    IF c_has_default THEN
      c_default := c->'default';

      -- Only allow JSON scalar defaults to avoid arbitrary SQL expressions.
      v_kind := jsonb_typeof(c_default);
      IF v_kind NOT IN ('string','number','boolean','null') THEN
        RAISE EXCEPTION 'default for column % must be a JSON scalar (string/number/boolean/null)', c_name;
      END IF;
    END IF;

    -- Start column definition: "<name> <type>"
    col_defs := col_defs
      || sep
      || format('%I %s', c_name, c_type::text);

    -- Nullability
    IF NOT c_nullable THEN
      col_defs := col_defs || ' NOT NULL';
    END IF;

    -- Default (literal only)
    IF c_has_default THEN
      IF jsonb_typeof(c_default) = 'null' THEN
        -- Explicit DEFAULT NULL is allowed but usually unnecessary; keep it deterministic.
        col_defs := col_defs || ' DEFAULT NULL';
      ELSE
        -- jsonb::text yields JSON formatting; for strings includes quotes, so we extract text for strings.
        IF jsonb_typeof(c_default) = 'string' THEN
          col_defs := col_defs || format(' DEFAULT %s', quote_literal(c_default #>> '{}'));
        ELSE
          -- number/boolean: render as text and quote as a literal (safe; PostgreSQL will cast if needed)
          col_defs := col_defs || format(' DEFAULT %s', quote_literal(c_default::text));
        END IF;
      END IF;
    END IF;

    -- UNIQUE (column-level)
    IF c_unique THEN
      col_defs := col_defs || ' UNIQUE';
    END IF;

    sep := ', ';
  END LOOP;

  ---------------------------------------------------------------------------
  -- 3) Create the table with standard columns + additional safe columns
  ---------------------------------------------------------------------------
  -- Note: status is assumed to be an existing type named "status".
  -- If it's in a different schema, qualify it (e.g. public.status) in your type definition.
  v_sql := format($fmt$
    CREATE TABLE IF NOT EXISTS %I (
      -- Standard identity primary key
      id INTEGER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,

      -- Your standard columns
      sys_client INT NOT NULL,
      status public.status NOT NULL,

      -- Extra columns from spec (may be empty)
      %s%s

      comment TEXT,
      tags JSONB NOT NULL DEFAULT '{}'::jsonb,
      sys_detail JSONB NOT NULL DEFAULT '{}'::jsonb,

      -- Audit columns (protected via privileges + trigger)
      created_by INT NOT NULL,
      updated_by INT NOT NULL,
      created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
      updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
    );
  $fmt$,
    v_table_name,
    CASE WHEN length(col_defs) > 0 THEN col_defs || ', ' ELSE '' END,
    ''  -- placeholder to keep formatting stable
  );

  EXECUTE v_sql;

  ---------------------------------------------------------------------------
  -- 4) Attach audit trigger (id/created_* immutable; updated_at maintained)
  ---------------------------------------------------------------------------
  -- Create trigger only if it doesn't already exist (idempotent).
  EXECUTE format($fmt$
    DO $do$
    BEGIN
      IF NOT EXISTS (
        SELECT 1
        FROM pg_trigger
        WHERE tgname = %L
          AND tgrelid = %L::regclass
      ) THEN
        CREATE TRIGGER %I
        BEFORE INSERT OR UPDATE ON %I
        FOR EACH ROW
        EXECUTE FUNCTION public.audit_enforce_timestamps_and_immutables();
      END IF;
    END
    $do$;
  $fmt$,
    v_table_name || '_audit_biu_trg',
    v_table_name,
    v_table_name || '_audit_biu_trg',
    v_table_name
  );

END;
$$;
