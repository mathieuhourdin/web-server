-- Retain exactly the removed associations for a coordinated rollback.
-- Cascades respect subsequent post/audience/account deletion.
CREATE TABLE removed_post_audiences_20261010 (
    post_id UUID NOT NULL REFERENCES posts(id) ON DELETE CASCADE,
    audience_id UUID NOT NULL REFERENCES audiences(id) ON DELETE CASCADE,
    is_default BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL,
    PRIMARY KEY (post_id, audience_id)
);

WITH removed AS (
    DELETE FROM post_audiences AS pa
    WHERE EXISTS (
        SELECT 1 FROM post_grants AS pg
        WHERE pg.post_id = pa.post_id
          AND pg.audience_id = pa.audience_id
    )
    AND NOT EXISTS (
        SELECT 1 FROM post_grants AS pg
        WHERE pg.post_id = pa.post_id
          AND pg.audience_id = pa.audience_id
          AND pg.status = 'ACTIVE'
    )
    RETURNING pa.post_id, pa.audience_id, pa.is_default, pa.created_at
)
INSERT INTO removed_post_audiences_20261010 (post_id, audience_id, is_default, created_at)
SELECT post_id, audience_id, is_default, created_at FROM removed;
