ALTER TABLE journal_sharing_policies
ADD COLUMN creation_origin TEXT NOT NULL DEFAULT 'MANUAL';

ALTER TABLE journal_sharing_policies
ADD CONSTRAINT journal_sharing_policies_creation_origin_check CHECK (
    creation_origin IN ('MANUAL', 'NEW_FOLLOWER', 'JOURNAL_SHARING_MODE')
);
