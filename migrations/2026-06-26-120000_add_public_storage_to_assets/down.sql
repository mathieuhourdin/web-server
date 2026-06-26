DROP INDEX IF EXISTS assets_public_object_key_idx;

ALTER TABLE assets
DROP COLUMN IF EXISTS public_object_key,
DROP COLUMN IF EXISTS public_bucket;
