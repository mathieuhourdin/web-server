-- Your SQL goes here
ALTER TABLE articles RENAME TO thought_outputs;

ALTER TABLE comments RENAME COLUMN article_id TO thought_output_id;
