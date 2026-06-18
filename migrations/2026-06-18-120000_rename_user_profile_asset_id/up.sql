DROP INDEX IF EXISTS users_profile_asset_id_idx;

ALTER TABLE users
RENAME COLUMN profile_asset_id TO profile_picture_asset_id;

CREATE INDEX IF NOT EXISTS users_profile_picture_asset_id_idx ON users(profile_picture_asset_id);
