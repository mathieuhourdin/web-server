CREATE TABLE devices (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    identifier TEXT NOT NULL,
    device_type TEXT NOT NULL,
    push_token TEXT,
    push_provider TEXT,
    name TEXT,
    app_version TEXT,
    os_name TEXT,
    os_version TEXT,
    browser_name TEXT,
    browser_version TEXT,
    last_seen_at TIMESTAMP,
    push_token_updated_at TIMESTAMP,
    revoked_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    CONSTRAINT devices_device_type_check CHECK (
        device_type IN ('IOS', 'ANDROID', 'WEB', 'DESKTOP', 'UNKNOWN')
    ),
    CONSTRAINT devices_push_provider_check CHECK (
        push_provider IS NULL OR push_provider IN ('APNS', 'FCM', 'WEB_PUSH', 'EXPO')
    ),
    CONSTRAINT devices_push_token_provider_check CHECK (
        push_token IS NULL OR push_provider IS NOT NULL
    ),
    CONSTRAINT devices_user_identifier_unique UNIQUE (user_id, identifier)
);

SELECT diesel_manage_updated_at('devices');

ALTER TABLE sessions
ADD COLUMN device_id UUID REFERENCES devices(id) ON DELETE SET NULL;

CREATE INDEX devices_user_id_idx
ON devices (user_id);

CREATE INDEX devices_user_active_idx
ON devices (user_id, revoked_at);

CREATE INDEX devices_push_token_idx
ON devices (push_provider, push_token)
WHERE push_token IS NOT NULL;

CREATE INDEX sessions_device_id_idx
ON sessions (device_id);
