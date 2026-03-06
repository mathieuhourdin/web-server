ALTER TABLE analysis_summaries
    ADD COLUMN short_content TEXT;

UPDATE analysis_summaries
SET short_content = content
WHERE short_content IS NULL;

ALTER TABLE analysis_summaries
    ALTER COLUMN short_content SET NOT NULL;

