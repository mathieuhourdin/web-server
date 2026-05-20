ALTER TABLE journal_share_links
ADD COLUMN scoped_post_id UUID NULL REFERENCES posts(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_journal_share_links_scoped_post_id
ON journal_share_links (scoped_post_id);
