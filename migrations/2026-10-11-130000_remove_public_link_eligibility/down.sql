-- Former exclusion values cannot be recovered after this intentional removal.
ALTER TABLE posts ADD COLUMN public_link_eligible BOOLEAN NOT NULL DEFAULT TRUE;
