ALTER TABLE users
ADD COLUMN ai_features_enabled_by_admin BOOLEAN NOT NULL DEFAULT TRUE;

CREATE INDEX users_ai_features_enabled_by_admin_idx
ON users (ai_features_enabled_by_admin);
