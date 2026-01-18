DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_type t
    JOIN pg_namespace n ON n.oid = t.typnamespace
    WHERE t.typname = 'status' AND n.nspname = 'app'
  ) THEN
    CREATE TYPE app.status AS ENUM ('incomplete', 'active', 'deleted', 'archived');
  END IF;
END
$$;
