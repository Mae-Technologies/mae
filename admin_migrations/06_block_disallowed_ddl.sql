-- Blocks table/sequence creation unless the executing role is allowed.
-- Install this in the "admin up-to-05" phase (superuser/admin connection).

CREATE OR REPLACE FUNCTION app.block_disallowed_ddl()
RETURNS event_trigger
LANGUAGE plpgsql
AS $$
DECLARE
  -- current_user is the *effective* role executing the DDL (SECURITY DEFINER => app_owner).
  effective_role text := current_user;
  invoker_role   text := session_user;
  -- required for preventing changes (DROPS, RENAMES) to default columns
  q text := current_query();
  protected_cols text[] := ARRAY[
    'id',
    'sys_client',
    'created_at',
    'created_by',
    'updated_at',
    'updated_by',
    'status',
    'sys_detail',
    'tags'
  ];
  col text;
BEGIN
  -- Allow superusers implicitly (Postgres bypasses many checks anyway),
  -- and allow your dedicated table_creator role.
  IF effective_role IN ('app_owner', 'postgres') THEN
    RETURN;
  END IF;

  -- Block direct table creation and common table-adjacent DDL.
  -- Add/remove tags here based on what you want to restrict.
  IF TG_TAG IN (
    'CREATE TABLE',
    'CREATE TABLE AS',
    'SELECT INTO',
    'CREATE SEQUENCE',
    'ALTER TABLE',
    'DROP TABLE',
    'DROP SEQUENCE'
  ) THEN
    RAISE EXCEPTION 'DDL "%": not allowed for role "%". Use create_table_from_spec/apply_table_acl.', TG_TAG, effective_role;
  END IF;


  -- Blocks ALTER TABLE that attempts to DROP or RENAME any "default" columns.
  -- Applies to tables in schema app only.
  --
  -- IMPORTANT:
  -- - This is a DDL event trigger; raising an exception aborts the DDL statement.
  -- - This relies on parsing current_query() for DROP/RENAME COLUMN patterns.
  --   This is robust enough for standard migrations, but avoid exotic formatting.
  -- Protect these default columns from DROP or RENAME.

  -- Only care about ALTER TABLE statements.
  IF TG_TAG <> 'ALTER TABLE' THEN
    RETURN;
  END IF;

  -- Only enforce for tables under schema app.
  -- This filters typical statements like:
  --   ALTER TABLE app.my_table ...
  -- and avoids blocking unrelated schemas.
  IF q !~* '\malter\s+table\s+app\.' THEN
    RETURN;
  END IF;

  -- Block DROP COLUMN on protected columns.
  FOREACH col IN ARRAY protected_cols LOOP
    -- Matches: DROP COLUMN <col> (optionally with IF EXISTS)
    IF q ~* format('\mdrop\s+column\s+(if\s+exists\s+)?%I\b', col) THEN
      RAISE EXCEPTION 'DDL blocked: cannot DROP protected column "%" on app tables', col;
    END IF;
  END LOOP;

  -- Block RENAME COLUMN <old> TO <new> where <old> is protected.
  FOREACH col IN ARRAY protected_cols LOOP
    IF q ~* format('\mrename\s+column\s+%I\s+to\b', col) THEN
      RAISE EXCEPTION 'DDL blocked: cannot RENAME protected column "%" on app tables', col;
    END IF;
  END LOOP;
END;
$$;

DROP EVENT TRIGGER IF EXISTS trg_block_disallowed_ddl;

CREATE EVENT TRIGGER trg_block_disallowed_ddl
ON ddl_command_start
EXECUTE FUNCTION app.block_disallowed_ddl();
