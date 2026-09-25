-- Keep existing users' stored choice intact. New accounts must explicitly
-- opt in through the existing ai_features_enabled preference.
ALTER TABLE users
ALTER COLUMN ai_features_enabled SET DEFAULT FALSE;
