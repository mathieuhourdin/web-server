ALTER TABLE analysis_summaries
    DROP COLUMN IF EXISTS meaningful_event_title,
    DROP COLUMN IF EXISTS meaningful_event_description,
    DROP COLUMN IF EXISTS meaningful_event_date;
