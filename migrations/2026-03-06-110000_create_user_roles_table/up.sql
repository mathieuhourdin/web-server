CREATE TABLE user_roles (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT now(),
    updated_at TIMESTAMP NOT NULL DEFAULT now(),
    UNIQUE (user_id, role),
    CHECK (role IN ('MEMBER', 'ADMIN', 'MENTOR'))
);

CREATE INDEX idx_user_roles_user_id ON user_roles(user_id);

CREATE TRIGGER set_updated_at
BEFORE UPDATE ON user_roles
FOR EACH ROW
EXECUTE FUNCTION diesel_set_updated_at();

INSERT INTO user_roles (user_id, role)
SELECT id, 'MEMBER'
FROM users
ON CONFLICT (user_id, role) DO NOTHING;
