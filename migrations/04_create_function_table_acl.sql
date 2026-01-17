-- Applies least-privilege ACL to a table with separate INSERT vs UPDATE allowlists.
-- Regular users:
--   - CAN: SELECT rows
--   - CAN: INSERT into allowed columns (includes sys_client so it can be set at creation time)
--   - CAN: UPDATE only allowed columns (sys_client excluded to keep it immutable)
--   - CANNOT: write id, created_by, updated_by, created_at, updated_at (or any non-allowlisted column)
CREATE OR REPLACE FUNCTION public.apply_table_acl(
  p_table_name TEXT,
  p_owner_role TEXT,
  p_user_role  TEXT,
  p_insertable_columns TEXT[], -- additional insertable columns; may be NULL/empty
  p_updatable_columns  TEXT[]  -- additional updatable columns; may be NULL/empty
)
RETURNS VOID
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
  -- Baseline insertable columns (sys_client is set at creation time).
  default_insertable TEXT[] := ARRAY['sys_client','status','comment','tags','sys_detail'];

  -- Baseline updatable columns (sys_client intentionally omitted; immutable after insert).
  default_updatable  TEXT[] := ARRAY['status','comment','tags','sys_detail'];

  final_insertable TEXT[];
  final_updatable  TEXT[];

  insert_list TEXT;
  update_list TEXT;
BEGIN
  -- Combine caller-provided allowlists with the baseline defaults.
  final_insertable := COALESCE(p_insertable_columns, ARRAY[]::TEXT[]) || default_insertable;
  final_updatable  := COALESCE(p_updatable_columns,  ARRAY[]::TEXT[]) || default_updatable;

  -- De-duplicate and make ordering deterministic.
  SELECT array_agg(DISTINCT c ORDER BY c) INTO final_insertable
  FROM unnest(final_insertable) AS t(c);

  SELECT array_agg(DISTINCT c ORDER BY c) INTO final_updatable
  FROM unnest(final_updatable) AS t(c);

  -- Transfer ownership to the privileged owner role.
  EXECUTE format('ALTER TABLE %I OWNER TO %I;', p_table_name, p_owner_role);

  -- Remove default/public privileges and any existing grants to the user role.
  EXECUTE format('REVOKE ALL ON TABLE %I FROM PUBLIC;', p_table_name);
  EXECUTE format('REVOKE ALL ON TABLE %I FROM %I;', p_table_name, p_user_role);

  -- Grant read access.
  EXECUTE format('GRANT SELECT ON TABLE %I TO %I;', p_table_name, p_user_role);

  -- Render column identifier lists for GRANT statements.
  SELECT string_agg(format('%I', c), ', ') INTO insert_list
  FROM unnest(final_insertable) AS t(c);

  SELECT string_agg(format('%I', c), ', ') INTO update_list
  FROM unnest(final_updatable) AS t(c);

  -- Allow INSERT only into insertable columns (sys_client allowed here).
  EXECUTE format('GRANT INSERT (%s) ON TABLE %I TO %I;', insert_list, p_table_name, p_user_role);

  -- Allow UPDATE only into updatable columns (sys_client excluded here).
  EXECUTE format('GRANT UPDATE (%s) ON TABLE %I TO %I;', update_list, p_table_name, p_user_role);

  -- Grant the runtime role permission to draw values from the identity sequence
  -- (required for INSERTs that rely on the default id).
  EXECUTE format('GRANT USAGE, SELECT ON SEQUENCE %I TO %I;',
                p_table_name || '_id_seq',
                p_user_role);
END;
$$;
