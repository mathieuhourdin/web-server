UPDATE relationships
SET status = 'REJECTED'
WHERE status = 'ARCHIVED';

ALTER TABLE relationships
DROP CONSTRAINT IF EXISTS relationships_status_check;

ALTER TABLE relationships
ADD CONSTRAINT relationships_status_check
CHECK (status IN ('PENDING', 'ACCEPTED', 'REJECTED', 'BLOCKED'));
