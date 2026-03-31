ALTER TABLE journal_grants
DROP CONSTRAINT IF EXISTS journal_grants_grantee_scope_check;

ALTER TABLE journal_grants
ADD CONSTRAINT journal_grants_grantee_scope_check
CHECK (
    grantee_scope IS NULL
    OR grantee_scope IN ('ALL_ACCEPTED_FOLLOWERS')
);

ALTER TABLE post_grants
DROP CONSTRAINT IF EXISTS post_grants_grantee_scope_check;

ALTER TABLE post_grants
ADD CONSTRAINT post_grants_grantee_scope_check
CHECK (
    grantee_scope IS NULL
    OR grantee_scope IN ('ALL_ACCEPTED_FOLLOWERS')
);
