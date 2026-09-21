CREATE TABLE wal_days (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    local_date DATE NOT NULL,
    input TEXT NOT NULL DEFAULT '',
    context TEXT NOT NULL DEFAULT '',
    compiled_operational TEXT NULL,
    compiled_thematic TEXT NULL,
    compiled_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT wal_days_unique_user_local_date UNIQUE (user_id, local_date)
);

SELECT diesel_manage_updated_at('wal_days');

INSERT INTO wal_days (
    id,
    user_id,
    local_date,
    input,
    context,
    compiled_operational,
    compiled_thematic,
    compiled_at,
    created_at,
    updated_at
)
SELECT
    uuid_generate_v4(),
    users.id,
    CASE
        WHEN users.timezone IN (SELECT name FROM pg_timezone_names)
            THEN timezone(users.timezone, NOW())::date
        ELSE timezone('UTC', NOW())::date
    END,
    users.wal_content,
    '',
    users.wal_compiled #>> '{operational,content}',
    users.wal_compiled #>> '{thematic,content}',
    COALESCE(
        NULLIF(users.wal_compiled #>> '{operational,compiled_at}', '')::timestamp,
        NULLIF(users.wal_compiled #>> '{thematic,compiled_at}', '')::timestamp
    ),
    NOW(),
    NOW()
FROM users
WHERE users.wal_content <> ''
   OR users.wal_compiled <> '{}'::jsonb
ON CONFLICT (user_id, local_date) DO NOTHING;
