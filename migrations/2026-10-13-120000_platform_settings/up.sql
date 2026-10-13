CREATE TABLE platform_settings (
    key TEXT PRIMARY KEY CHECK (length(trim(key)) > 0),
    value JSONB NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL
);
-- No policy is seeded: existing mobile versions must not be blocked implicitly.
