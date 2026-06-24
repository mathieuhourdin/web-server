CREATE TABLE journal_sharing_policies (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    journal_id UUID NOT NULL REFERENCES journals(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    grantee_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    default_future_access_enabled BOOLEAN NOT NULL DEFAULT false,
    history_review_state TEXT NOT NULL,
    history_decision TEXT,
    history_reviewed_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    updated_at TIMESTAMP NOT NULL DEFAULT current_timestamp,
    CONSTRAINT journal_sharing_policies_status_check CHECK (
        status IN ('SUGGESTED', 'ACTIVE', 'DISABLED', 'DISMISSED', 'REVOKED')
    ),
    CONSTRAINT journal_sharing_policies_history_review_state_check CHECK (
        history_review_state IN ('NOT_STARTED', 'UNREVIEWED', 'REVIEWED')
    ),
    CONSTRAINT journal_sharing_policies_history_decision_check CHECK (
        history_decision IS NULL OR history_decision IN ('NONE', 'ALL', 'NORMAL_ONLY', 'SPECIFIC')
    ),
    CONSTRAINT journal_sharing_policies_journal_grantee_unique UNIQUE (journal_id, grantee_user_id)
);

SELECT diesel_manage_updated_at('journal_sharing_policies');

CREATE INDEX journal_sharing_policies_journal_id_idx
ON journal_sharing_policies (journal_id);

CREATE INDEX journal_sharing_policies_owner_grantee_idx
ON journal_sharing_policies (owner_user_id, grantee_user_id);

CREATE INDEX journal_sharing_policies_active_defaults_idx
ON journal_sharing_policies (journal_id, status, default_future_access_enabled)
WHERE status = 'ACTIVE' AND default_future_access_enabled = true;

INSERT INTO journal_sharing_policies (
    id,
    journal_id,
    owner_user_id,
    grantee_user_id,
    status,
    default_future_access_enabled,
    history_review_state,
    history_decision,
    history_reviewed_at,
    created_at,
    updated_at
)
SELECT
    uuid_generate_v4(),
    jg.journal_id,
    jg.owner_user_id,
    jg.grantee_user_id,
    CASE WHEN jg.status = 'ACTIVE' THEN 'ACTIVE' ELSE 'REVOKED' END,
    CASE WHEN jg.status = 'ACTIVE' THEN true ELSE false END,
    'REVIEWED',
    'ALL',
    jg.updated_at,
    jg.created_at,
    jg.updated_at
FROM journal_grants jg
WHERE jg.grantee_user_id IS NOT NULL
ON CONFLICT (journal_id, grantee_user_id) DO UPDATE SET
    status = EXCLUDED.status,
    default_future_access_enabled = EXCLUDED.default_future_access_enabled,
    history_review_state = EXCLUDED.history_review_state,
    history_decision = EXCLUDED.history_decision,
    history_reviewed_at = EXCLUDED.history_reviewed_at,
    updated_at = EXCLUDED.updated_at;

INSERT INTO journal_sharing_policies (
    id,
    journal_id,
    owner_user_id,
    grantee_user_id,
    status,
    default_future_access_enabled,
    history_review_state,
    history_decision,
    history_reviewed_at,
    created_at,
    updated_at
)
SELECT
    uuid_generate_v4(),
    jg.journal_id,
    jg.owner_user_id,
    r.requester_user_id,
    CASE WHEN jg.status = 'ACTIVE' THEN 'ACTIVE' ELSE 'REVOKED' END,
    CASE WHEN jg.status = 'ACTIVE' THEN true ELSE false END,
    'REVIEWED',
    'ALL',
    jg.updated_at,
    jg.created_at,
    jg.updated_at
FROM journal_grants jg
JOIN relationships r
    ON r.target_user_id = jg.owner_user_id
   AND r.relationship_type = 'FOLLOW'
   AND r.status = 'ACCEPTED'
WHERE jg.grantee_scope = 'ALL_ACCEPTED_FOLLOWERS'
ON CONFLICT (journal_id, grantee_user_id) DO NOTHING;

INSERT INTO journal_sharing_policies (
    id,
    journal_id,
    owner_user_id,
    grantee_user_id,
    status,
    default_future_access_enabled,
    history_review_state,
    history_decision,
    history_reviewed_at,
    created_at,
    updated_at
)
SELECT
    uuid_generate_v4(),
    jg.journal_id,
    jg.owner_user_id,
    u.id,
    CASE WHEN jg.status = 'ACTIVE' THEN 'ACTIVE' ELSE 'REVOKED' END,
    CASE WHEN jg.status = 'ACTIVE' THEN true ELSE false END,
    'REVIEWED',
    'ALL',
    jg.updated_at,
    jg.created_at,
    jg.updated_at
FROM journal_grants jg
JOIN users u
    ON u.is_platform_user = true
   AND u.principal_type = 'HUMAN'
   AND u.id <> jg.owner_user_id
WHERE jg.grantee_scope = 'ALL_PLATFORM_USERS'
ON CONFLICT (journal_id, grantee_user_id) DO NOTHING;

DROP TABLE journal_grants;
