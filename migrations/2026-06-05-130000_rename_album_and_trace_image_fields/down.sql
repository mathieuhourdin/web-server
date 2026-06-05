ALTER INDEX IF EXISTS traces_content_image_asset_id_idx
RENAME TO traces_image_asset_id_idx;

ALTER TABLE trace_versions
RENAME COLUMN content_image_asset_id TO image_asset_id;

ALTER TABLE traces
RENAME COLUMN content_image_asset_id TO image_asset_id;

ALTER TABLE albums
RENAME COLUMN cover_image_asset_id TO cover_asset_id;
