DROP INDEX IF EXISTS users_ai_features_enabled_by_admin_idx;

ALTER TABLE users
DROP COLUMN ai_features_enabled_by_admin;
