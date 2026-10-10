-- Coordinated rollback: restore only removed associations whose entities still
-- exist, without overwriting associations explicitly recreated since cleanup.
-- This restores history-sharing scope, not grants or granted access.
INSERT INTO post_audiences (post_id, audience_id, is_default, created_at)
SELECT post_id, audience_id, is_default, created_at
FROM removed_post_audiences_20261010
ON CONFLICT (post_id, audience_id) DO NOTHING;

DROP TABLE removed_post_audiences_20261010;
