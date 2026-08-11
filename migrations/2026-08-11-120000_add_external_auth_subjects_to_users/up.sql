ALTER TABLE users
ADD COLUMN google_sub TEXT,
ADD COLUMN apple_sub TEXT;

CREATE UNIQUE INDEX users_google_sub_unique
ON users (google_sub)
WHERE google_sub IS NOT NULL;

CREATE UNIQUE INDEX users_apple_sub_unique
ON users (apple_sub)
WHERE apple_sub IS NOT NULL;
