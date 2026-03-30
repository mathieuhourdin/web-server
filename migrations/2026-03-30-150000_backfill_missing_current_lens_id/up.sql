WITH latest_lenses AS (
    SELECT DISTINCT ON (user_id)
        user_id,
        id
    FROM lenses
    ORDER BY user_id, created_at DESC, id DESC
)
UPDATE users
SET current_lens_id = latest_lenses.id
FROM latest_lenses
WHERE users.id = latest_lenses.user_id
  AND users.current_lens_id IS NULL;
