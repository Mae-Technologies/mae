DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_owner') THEN
    CREATE ROLE app_owner NOLOGIN;
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_user') THEN
    CREATE ROLE app_user NOLOGIN;
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_migrator') THEN
    CREATE ROLE app_migrator NOLOGIN;
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'table_creator') THEN
    CREATE ROLE table_creator NOLOGIN;
  END IF;

  -- Create schema owned by app_owner
  CREATE SCHEMA IF NOT EXISTS app AUTHORIZATION app_owner;

  -- Lock down public
  REVOKE CREATE ON SCHEMA public FROM PUBLIC;
  -- Only revoke from a role if you actually have that role; using app_migrator here for consistency
  REVOKE CREATE ON SCHEMA public FROM app_migrator;

  -- Allow migrator / table creator to create in app schema
  GRANT USAGE, CREATE ON SCHEMA app TO app_migrator;
  GRANT USAGE, CREATE ON SCHEMA app TO table_creator;

  -- Be explicit
  GRANT USAGE, CREATE ON SCHEMA app TO app_owner;
  GRANT USAGE ON SCHEMA app TO app_user;

  -- Set search_path
  ALTER ROLE app_owner SET search_path = app, public;
  ALTER ROLE app_migrator SET search_path = app;
  ALTER ROLE app_user SET search_path = app;
  ALTER ROLE table_creator SET search_path = app;
END
$$;
