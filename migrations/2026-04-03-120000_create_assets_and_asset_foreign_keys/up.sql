CREATE TABLE assets (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    bucket TEXT NOT NULL,
    object_key TEXT NOT NULL UNIQUE,
    mime_type TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX assets_owner_user_id_idx ON assets(owner_user_id);

ALTER TABLE users
ADD COLUMN profile_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL;

CREATE INDEX users_profile_asset_id_idx ON users(profile_asset_id);

ALTER TABLE posts
ADD COLUMN image_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL;

CREATE INDEX posts_image_asset_id_idx ON posts(image_asset_id);

ALTER TABLE traces
ADD COLUMN image_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL;

CREATE INDEX traces_image_asset_id_idx ON traces(image_asset_id);
