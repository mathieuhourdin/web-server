ALTER TABLE posts
    ADD COLUMN notification_mode TEXT NOT NULL DEFAULT 'ALL'
        CHECK (notification_mode IN ('ALL', 'NONE', 'SELECTED')),
    ADD COLUMN notification_user_ids UUID[] NOT NULL DEFAULT '{}',
    ADD CONSTRAINT posts_notification_selection_check CHECK (
        (notification_mode = 'SELECTED' AND cardinality(notification_user_ids) BETWEEN 1 AND 500)
        OR (notification_mode <> 'SELECTED' AND cardinality(notification_user_ids) = 0)
    );
