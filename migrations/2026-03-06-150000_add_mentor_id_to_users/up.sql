ALTER TABLE users
ADD COLUMN mentor_id UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX idx_users_mentor_id ON users (mentor_id);
