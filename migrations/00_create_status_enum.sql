DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_type t
    JOIN pg_namespace n ON n.oid = t.typnamespace
    WHERE t.typname = 'status' AND n.nspname = 'public'
  ) THEN
    CREATE TYPE public.status AS ENUM ('incomplete', 'active', 'deleted', 'archived');
  END IF;
END
$$;
