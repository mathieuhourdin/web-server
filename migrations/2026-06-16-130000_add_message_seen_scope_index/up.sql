CREATE INDEX IF NOT EXISTS idx_messages_unseen_received_scope
ON messages (
    recipient_user_id,
    sender_user_id,
    post_id,
    trace_id,
    created_at DESC
)
WHERE seen_at IS NULL
  AND processing_state = 'PROCESSED';
