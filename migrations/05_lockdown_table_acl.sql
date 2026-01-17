-- Lock down apply_table_acl; allow only table_creator to call it.
REVOKE ALL ON FUNCTION public.apply_table_acl(TEXT, TEXT, TEXT, TEXT[], TEXT[]) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.apply_table_acl(TEXT, TEXT, TEXT, TEXT[], TEXT[]) TO table_creator;
ALTER FUNCTION public.apply_table_acl(TEXT, TEXT, TEXT, TEXT[], TEXT[]) OWNER TO app_owner;

-- Lock down execution of the factory function.
REVOKE ALL ON FUNCTION public.create_table_from_spec(jsonb) FROM PUBLIC;
GRANT EXECUTE ON FUNCTION public.create_table_from_spec(jsonb) TO table_creator;

-- Ensure the function is owned by app_owner (so SECURITY DEFINER runs as app_owner).
ALTER FUNCTION public.create_table_from_spec(jsonb) OWNER TO app_owner;

-- Lock down audit trigger function (only needs to run via trigger; not callable by PUBLIC).
REVOKE ALL ON FUNCTION public.audit_enforce_timestamps_and_immutables() FROM PUBLIC;
ALTER FUNCTION public.audit_enforce_timestamps_and_immutables() OWNER TO app_owner;

