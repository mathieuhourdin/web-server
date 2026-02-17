WITH missing_users AS (
    SELECT u.id AS user_id
    FROM users u
    WHERE NOT EXISTS (
        SELECT 1
        FROM interactions i
        INNER JOIN resources r ON r.id = i.resource_id
        WHERE i.interaction_user_id = u.id
          AND i.interaction_type = 'outp'
          AND r.resource_type = 'meta'
    )
),
prepared AS (
    SELECT user_id, uuid_generate_v4() AS resource_id
    FROM missing_users
),
inserted_resources AS (
    INSERT INTO resources (
        id,
        title,
        subtitle,
        content,
        external_content_url,
        comment,
        image_url,
        resource_type,
        maturing_state,
        publishing_state,
        category_id,
        is_external,
        created_at,
        updated_at,
        entity_type
    )
    SELECT
        p.resource_id,
        'Meta Journal',
        '',
        '',
        NULL,
        NULL,
        NULL,
        'meta',
        'drft',
        'drft',
        NULL,
        false,
        now(),
        now(),
        'jrnl'
    FROM prepared p
    RETURNING id
)
INSERT INTO interactions (
    interaction_user_id,
    interaction_progress,
    interaction_comment,
    interaction_date,
    interaction_type,
    interaction_is_public,
    resource_id,
    created_at,
    updated_at
)
SELECT
    p.user_id,
    100,
    NULL,
    now(),
    'outp',
    true,
    p.resource_id,
    now(),
    now()
FROM prepared p;
