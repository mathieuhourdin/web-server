ALTER TABLE users
ADD COLUMN IF NOT EXISTS mentor_feedback_email_enabled BOOLEAN;

UPDATE users
SET mentor_feedback_email_enabled = TRUE
WHERE mentor_feedback_email_enabled IS NULL;

ALTER TABLE users
ALTER COLUMN mentor_feedback_email_enabled SET DEFAULT TRUE;

ALTER TABLE users
ALTER COLUMN mentor_feedback_email_enabled SET NOT NULL;
