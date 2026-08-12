ALTER TABLE assets
DROP CONSTRAINT IF EXISTS assets_image_dimensions_pair_check,
DROP COLUMN image_height,
DROP COLUMN image_width;
