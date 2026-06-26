ALTER TABLE assets
ADD COLUMN public_bucket TEXT NULL,
ADD COLUMN public_object_key TEXT NULL;

CREATE INDEX IF NOT EXISTS assets_public_object_key_idx
ON assets(public_bucket, public_object_key)
WHERE public_object_key IS NOT NULL;
