DROP TRIGGER a_validate_channel_publication ON posts;
CREATE TRIGGER a_validate_channel_publication BEFORE INSERT OR UPDATE ON posts
    FOR EACH ROW EXECUTE FUNCTION validate_channel_publication();
DROP INDEX posts_channel_ordering_idx;
ALTER TABLE posts DROP CONSTRAINT posts_channel_ordering_date_check;
ALTER TABLE posts DROP COLUMN ordering_date;
