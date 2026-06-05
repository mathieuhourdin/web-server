ALTER TABLE posts
ADD COLUMN content_source TEXT NOT NULL DEFAULT 'CUSTOM',
ADD COLUMN image_asset_id UUID NULL REFERENCES assets(id) ON DELETE SET NULL,
ADD COLUMN maturing_state TEXT NOT NULL DEFAULT 'drft',
ADD COLUMN publishing_state TEXT NOT NULL DEFAULT 'pbsh',
ADD COLUMN image_url TEXT NULL;

CREATE INDEX IF NOT EXISTS posts_image_asset_id_idx ON posts(image_asset_id);
