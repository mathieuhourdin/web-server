ALTER TABLE analysis_summaries
    ADD COLUMN activity_emoji TEXT,
    ADD COLUMN mood_emoji TEXT,
    ADD COLUMN dominant_emoji_kind TEXT CHECK (dominant_emoji_kind IN ('ACTIVITY', 'MOOD'));
