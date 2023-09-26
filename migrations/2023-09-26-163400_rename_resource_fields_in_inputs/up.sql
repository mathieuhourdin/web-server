-- Your SQL goes here
ALTER TABLE thought_inputs ADD COLUMN resource_subtitle TEXT NOT NULL DEFAULT '';
ALTER TABLE thought_inputs ADD COLUMN resource_content TEXT NOT NULL DEFAULT '';
ALTER TABLE thought_inputs ADD COLUMN resource_category_id UUID REFERENCES categories(id);
ALTER TABLE thought_inputs RENAME COLUMN resource_link TO resource_external_content_url;
ALTER TABLE thought_inputs RENAME COLUMN resource_image_link TO resource_image_url;
