ALTER TABLE documents
DROP COLUMN IF EXISTS cover_image_external_url,
DROP COLUMN IF EXISTS cover_image_asset_id;
