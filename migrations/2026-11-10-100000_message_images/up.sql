-- Conversation media is intentionally separate from permanent platform assets.
CREATE TABLE message_images (
    id uuid PRIMARY KEY,
    conversation_id uuid NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    uploaded_by_user_id uuid NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    message_id uuid REFERENCES messages(id) ON DELETE SET NULL,
    position integer,
    bucket text NOT NULL,
    object_key text NOT NULL UNIQUE,
    mime_type text NOT NULL CHECK (mime_type IN ('image/jpeg','image/png','image/webp')),
    size_bytes bigint NOT NULL CHECK (size_bytes BETWEEN 1 AND 5242880),
    width integer NOT NULL CHECK (width > 0),
    height integer NOT NULL CHECK (height > 0),
    status text NOT NULL CHECK (status IN ('UPLOADING','READY','FAILED')),
    created_at timestamp NOT NULL DEFAULT timezone('UTC',now()),
    expires_at timestamp NOT NULL DEFAULT (timezone('UTC',now()) + interval '45 days'),
    CHECK (expires_at > created_at),
    CHECK (message_id IS NULL OR (status='READY' AND position BETWEEN 0 AND 3)),
    UNIQUE(message_id,position)
);
CREATE INDEX message_images_uploader_created ON message_images(uploaded_by_user_id,created_at);
CREATE INDEX message_images_conversation ON message_images(conversation_id);
