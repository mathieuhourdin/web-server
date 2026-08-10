UPDATE journal_sharing_policies AS policy
SET history_review_state = 'REVIEWED',
    history_decision = 'NONE',
    history_reviewed_at = COALESCE(policy.history_reviewed_at, NOW()),
    updated_at = NOW()
WHERE policy.creation_origin IN ('NEW_FOLLOWER', 'JOURNAL_SHARING_MODE')
  AND policy.status = 'ACTIVE'
  AND policy.history_review_state = 'UNREVIEWED'
  AND NOT EXISTS (
      SELECT 1
      FROM posts
      INNER JOIN traces ON posts.source_trace_id = traces.id
      WHERE posts.user_id = policy.owner_user_id
        AND posts.status <> 'ARCHIVED'
        AND traces.journal_id = policy.journal_id
        AND traces.trace_type = 'USER_TRACE'
        AND traces.status <> 'ARCHIVED'
  );
