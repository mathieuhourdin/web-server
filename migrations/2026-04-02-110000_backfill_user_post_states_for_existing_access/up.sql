INSERT INTO user_post_states (
    user_id,
    post_id,
    first_seen_at,
    last_seen_at,
    created_at,
    updated_at
)
SELECT
    p.user_id,
    p.id,
    COALESCE(p.publishing_date, p.updated_at, p.created_at),
    COALESCE(p.publishing_date, p.updated_at, p.created_at),
    NOW(),
    NOW()
FROM posts p
ON CONFLICT (user_id, post_id) DO NOTHING;

INSERT INTO user_post_states (
    user_id,
    post_id,
    first_seen_at,
    last_seen_at,
    created_at,
    updated_at
)
SELECT DISTINCT
    pg.grantee_user_id,
    p.id,
    COALESCE(p.publishing_date, p.updated_at, p.created_at),
    COALESCE(p.publishing_date, p.updated_at, p.created_at),
    NOW(),
    NOW()
FROM posts p
JOIN post_grants pg ON pg.post_id = p.id
WHERE p.status = 'PUBLISHED'
  AND pg.status = 'ACTIVE'
  AND pg.grantee_user_id IS NOT NULL
ON CONFLICT (user_id, post_id) DO NOTHING;

INSERT INTO user_post_states (
    user_id,
    post_id,
    first_seen_at,
    last_seen_at,
    created_at,
    updated_at
)
SELECT DISTINCT
    r.requester_user_id,
    p.id,
    COALESCE(p.publishing_date, p.updated_at, p.created_at),
    COALESCE(p.publishing_date, p.updated_at, p.created_at),
    NOW(),
    NOW()
FROM posts p
JOIN post_grants pg ON pg.post_id = p.id
JOIN relationships r
  ON r.target_user_id = p.user_id
 AND r.status = 'ACCEPTED'
WHERE p.status = 'PUBLISHED'
  AND pg.status = 'ACTIVE'
  AND pg.grantee_scope = 'ALL_ACCEPTED_FOLLOWERS'
ON CONFLICT (user_id, post_id) DO NOTHING;

INSERT INTO user_post_states (
    user_id,
    post_id,
    first_seen_at,
    last_seen_at,
    created_at,
    updated_at
)
SELECT DISTINCT
    u.id,
    p.id,
    COALESCE(p.publishing_date, p.updated_at, p.created_at),
    COALESCE(p.publishing_date, p.updated_at, p.created_at),
    NOW(),
    NOW()
FROM posts p
JOIN post_grants pg ON pg.post_id = p.id
JOIN users u
  ON u.is_platform_user = TRUE
 AND u.principal_type = 'HUMAN'
WHERE p.status = 'PUBLISHED'
  AND pg.status = 'ACTIVE'
  AND pg.grantee_scope = 'ALL_PLATFORM_USERS'
ON CONFLICT (user_id, post_id) DO NOTHING;
