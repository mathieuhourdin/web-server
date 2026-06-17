DROP INDEX IF EXISTS sessions_device_id_idx;
DROP INDEX IF EXISTS devices_push_token_idx;
DROP INDEX IF EXISTS devices_user_active_idx;
DROP INDEX IF EXISTS devices_user_id_idx;

ALTER TABLE sessions
DROP COLUMN IF EXISTS device_id;

DROP TABLE IF EXISTS devices;
