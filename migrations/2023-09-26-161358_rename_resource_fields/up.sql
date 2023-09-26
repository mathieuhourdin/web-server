-- Your SQL goes here
ALTER TABLE thought_outputs RENAME COLUMN title TO resource_title;
ALTER TABLE thought_outputs RENAME COLUMN description TO resource_subtitle;
ALTER TABLE thought_outputs RENAME COLUMN content TO resource_content;
ALTER TABLE thought_outputs RENAME COLUMN potential_improvements TO resource_comment;
ALTER TABLE thought_outputs RENAME COLUMN gdoc_url TO resource_external_content_url;
ALTER TABLE thought_outputs RENAME COLUMN image_url TO resource_image_url;
ALTER TABLE thought_outputs RENAME COLUMN output_type TO resource_type;
ALTER TABLE thought_outputs RENAME COLUMN category_id TO resource_category_id ;

