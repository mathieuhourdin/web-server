ALTER TABLE assets
ADD COLUMN image_width INTEGER,
ADD COLUMN image_height INTEGER;

ALTER TABLE assets
ADD CONSTRAINT assets_image_dimensions_pair_check CHECK (
    (image_width IS NULL AND image_height IS NULL)
    OR (image_width > 0 AND image_height > 0)
);
