DROP INDEX IF EXISTS traces_image_asset_id_idx;
ALTER TABLE traces DROP COLUMN IF EXISTS image_asset_id;

DROP INDEX IF EXISTS posts_image_asset_id_idx;
ALTER TABLE posts DROP COLUMN IF EXISTS image_asset_id;

DROP INDEX IF EXISTS users_profile_asset_id_idx;
ALTER TABLE users DROP COLUMN IF EXISTS profile_asset_id;

DROP INDEX IF EXISTS assets_owner_user_id_idx;
DROP TABLE IF EXISTS assets;
