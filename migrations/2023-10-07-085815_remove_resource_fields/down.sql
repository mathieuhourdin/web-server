-- This file should undo anything in `up.sql`
ALTER TABLE interactions ADD COLUMN resource_title TEXT NOT NULL DEFAULT '';
ALTER TABLE interactions ADD COLUMN resource_subtitle TEXT NOT NULL DEFAULT '';
ALTER TABLE interactions ADD COLUMN resource_content TEXT NOT NULL DEFAULT '';
ALTER TABLE interactions ADD COLUMN resource_external_content_url TEXT;
ALTER TABLE interactions ADD COLUMN resource_comment TEXT NOT NULL DEFAULT '';
ALTER TABLE interactions ADD COLUMN resource_maturing_state TEXT NOT NULL DEFAULT '';
ALTER TABLE interactions ADD COLUMN resource_publishing_state TEXT NOT NULL DEFAULT '';
ALTER TABLE interactions ADD COLUMN resource_parent_id UUID;
ALTER TABLE interactions ADD COLUMN resource_image_url TEXT;
ALTER TABLE interactions ADD COLUMN resource_type TEXT NOT NULL DEFAULT '';
ALTER TABLE interactions ADD COLUMN resource_category_id TEXT;
