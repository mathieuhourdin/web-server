-- This file should undo anything in `up.sql`
ALTER TABLE thought_inputs DROP COLUMN resource_subtitle;
ALTER TABLE thought_inputs DROP COLUMN resource_content;
ALTER TABLE thought_inputs DROP COLUMN resource_category_id;
ALTER TABLE thought_inputs RENAME COLUMN resource_external_content_url TO resource_link;
ALTER TABLE thought_inputs RENAME COLUMN resource_image_url TO resource_image_link;
