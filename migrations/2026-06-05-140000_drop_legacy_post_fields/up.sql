DROP INDEX IF EXISTS posts_image_asset_id_idx;

ALTER TABLE posts
DROP COLUMN IF EXISTS image_url,
DROP COLUMN IF EXISTS publishing_state,
DROP COLUMN IF EXISTS maturing_state,
DROP COLUMN IF EXISTS image_asset_id,
DROP COLUMN IF EXISTS content_source;
