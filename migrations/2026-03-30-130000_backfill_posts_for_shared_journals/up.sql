INSERT INTO posts (
    user_id,
    source_trace_id,
    title,
    subtitle,
    content,
    image_url,
    interaction_type,
    post_type,
    publishing_date,
    publishing_state,
    maturing_state,
    created_at,
    updated_at
)
SELECT
    t.user_id,
    t.id,
    t.title,
    t.subtitle,
    t.content,
    NULL,
    'OUTPUT',
    'IDEA',
    COALESCE(t.finalized_at, t.interaction_date),
    'pbsh',
    'fnsh',
    COALESCE(t.finalized_at, t.updated_at, t.created_at),
    COALESCE(t.finalized_at, t.updated_at, t.created_at)
FROM traces t
JOIN journals j ON j.id = t.journal_id
WHERE t.trace_type = 'USER_TRACE'
  AND t.status = 'FINALIZED'
  AND j.is_encrypted = FALSE
  AND j.status <> 'ARCHIVED'
  AND EXISTS (
      SELECT 1
      FROM journal_grants jg
      WHERE jg.journal_id = j.id
        AND jg.status = 'ACTIVE'
  )
  AND NOT EXISTS (
      SELECT 1
      FROM posts p
      WHERE p.source_trace_id = t.id
  );

INSERT INTO post_grants (
    id,
    post_id,
    owner_user_id,
    grantee_user_id,
    grantee_scope,
    access_level,
    status,
    created_at,
    updated_at
)
SELECT
    uuid_generate_v4(),
    p.id,
    p.user_id,
    jg.grantee_user_id,
    jg.grantee_scope,
    'READ',
    'ACTIVE',
    COALESCE(p.publishing_date, p.created_at),
    COALESCE(p.publishing_date, p.updated_at)
FROM posts p
JOIN traces t ON t.id = p.source_trace_id
JOIN journal_grants jg ON jg.journal_id = t.journal_id
WHERE jg.status = 'ACTIVE'
  AND NOT EXISTS (
      SELECT 1
      FROM post_grants pg
      WHERE pg.post_id = p.id
        AND (
            (pg.grantee_user_id IS NOT DISTINCT FROM jg.grantee_user_id)
            AND (pg.grantee_scope IS NOT DISTINCT FROM jg.grantee_scope)
        )
  );
