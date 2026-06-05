ALTER TABLE albums
RENAME COLUMN cover_asset_id TO cover_image_asset_id;

ALTER TABLE traces
RENAME COLUMN image_asset_id TO content_image_asset_id;

ALTER TABLE trace_versions
RENAME COLUMN image_asset_id TO content_image_asset_id;

ALTER INDEX IF EXISTS traces_image_asset_id_idx
RENAME TO traces_content_image_asset_id_idx;
