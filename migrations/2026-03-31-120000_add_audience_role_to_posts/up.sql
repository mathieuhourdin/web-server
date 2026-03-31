ALTER TABLE posts
ADD COLUMN audience_role TEXT NOT NULL DEFAULT 'DEFAULT';

UPDATE posts
SET audience_role = 'DEFAULT'
WHERE audience_role IS NULL;

