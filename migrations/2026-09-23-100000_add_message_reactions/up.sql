CREATE TABLE message_reactions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    message_id UUID NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    emoji TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT message_reactions_emoji_check CHECK (
        CHAR_LENGTH(BTRIM(emoji)) BETWEEN 1 AND 16
    ),
    CONSTRAINT message_reactions_message_user_unique UNIQUE (message_id, user_id)
);

CREATE INDEX message_reactions_message_created_idx
    ON message_reactions (message_id, created_at);

SELECT diesel_manage_updated_at('message_reactions');
