-- This file should undo anything in `up.sql`
ALTER TABLE thought_outputs RENAME COLUMN resource_title TO title;
ALTER TABLE thought_outputs RENAME COLUMN resource_subtitle TO description;
ALTER TABLE thought_outputs RENAME COLUMN resource_content TO content;
ALTER TABLE thought_outputs RENAME COLUMN resource_comment TO potential_improvements ;
ALTER TABLE thought_outputs RENAME COLUMN resource_external_content_url TO gdoc_url;
ALTER TABLE thought_outputs RENAME COLUMN resource_image_url TO image_url ;
ALTER TABLE thought_outputs RENAME COLUMN resource_type TO output_type;
ALTER TABLE thought_outputs RENAME COLUMN resource_category_id TO category_id ;
