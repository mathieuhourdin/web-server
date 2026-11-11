-- One immutable alias per channel publication. Backfilling URL metadata must not
-- republish or revalidate old content, so it doesn't update the posts table.
CREATE TABLE blog_publication_links (
    publication_id uuid PRIMARY KEY REFERENCES posts(id) ON DELETE CASCADE,
    code text NOT NULL UNIQUE CHECK (code ~ '^[A-Za-z0-9_-]{8}$')
);

CREATE FUNCTION allocate_blog_publication_link(publication uuid) RETURNS void LANGUAGE plpgsql AS $$
DECLARE attempt integer; candidate text;
BEGIN
    IF NOT EXISTS(SELECT 1 FROM posts WHERE id=publication AND publication_channel_id IS NOT NULL) THEN
        RAISE EXCEPTION 'Short links require a channel publication' USING ERRCODE='23514';
    END IF;
    IF EXISTS(SELECT 1 FROM blog_publication_links WHERE publication_id=publication) THEN RETURN; END IF;
    FOR attempt IN 1..8 LOOP
        -- Independent random UUID, not the publication ID. Its first six bytes
        -- supply 48 random bits, encoded as eight URL-safe characters. No extension.
        candidate := translate(substr(encode(uuid_send(gen_random_uuid()),'base64'),1,8),'+/','-_');
        INSERT INTO blog_publication_links(publication_id,code) VALUES(publication,candidate)
            ON CONFLICT DO NOTHING;
        IF FOUND OR EXISTS(SELECT 1 FROM blog_publication_links WHERE publication_id=publication) THEN RETURN; END IF;
    END LOOP;
    RAISE EXCEPTION 'Unable to allocate a unique publication short link' USING ERRCODE='23505';
END $$;
CREATE FUNCTION assign_blog_publication_link() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM allocate_blog_publication_link(NEW.id);
    RETURN NULL;
END $$;
CREATE TRIGGER assign_blog_publication_link AFTER INSERT ON posts
    FOR EACH ROW WHEN (NEW.publication_channel_id IS NOT NULL)
    EXECUTE FUNCTION assign_blog_publication_link();

-- Includes archived publications; does not change publication dates or sharing.
SELECT allocate_blog_publication_link(id) FROM posts WHERE publication_channel_id IS NOT NULL;

CREATE FUNCTION preserve_blog_publication_link() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW IS DISTINCT FROM OLD THEN
        RAISE EXCEPTION 'Publication short link is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER preserve_blog_publication_link BEFORE UPDATE ON blog_publication_links
    FOR EACH ROW EXECUTE FUNCTION preserve_blog_publication_link();
