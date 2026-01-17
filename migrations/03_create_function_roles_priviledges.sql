-- Create roles once (idempotent).
-- app_owner: owns tables/functions; no direct login.
-- app_user: granted to login roles; limited table privileges.
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_owner') THEN
    CREATE ROLE app_owner NOLOGIN;
  END IF;

  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'app_user') THEN
    CREATE ROLE app_user NOLOGIN;
  END IF;
END
$$;

-- Create a role that can only execute the table-factory function (idempotent).
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'table_creator') THEN
    CREATE ROLE table_creator NOLOGIN;
  END IF;
END
$$;
