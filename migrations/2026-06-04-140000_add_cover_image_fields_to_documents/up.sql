ALTER TABLE documents
ADD COLUMN cover_image_asset_id UUID REFERENCES assets(id),
ADD COLUMN cover_image_external_url TEXT;
