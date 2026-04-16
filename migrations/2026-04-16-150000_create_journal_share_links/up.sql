CREATE TABLE IF NOT EXISTS journal_share_links (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    journal_id UUID NOT NULL REFERENCES journals(id) ON DELETE CASCADE,
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMP NOT NULL,
    revoked_at TIMESTAMP NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_journal_share_links_journal_id
ON journal_share_links (journal_id);

CREATE INDEX IF NOT EXISTS idx_journal_share_links_owner_user_id
ON journal_share_links (owner_user_id);

CREATE INDEX IF NOT EXISTS idx_journal_share_links_expires_at
ON journal_share_links (expires_at);
