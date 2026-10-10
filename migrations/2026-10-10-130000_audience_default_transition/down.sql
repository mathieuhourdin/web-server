CREATE OR REPLACE FUNCTION materialize_post_audiences() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE source traces%ROWTYPE; aid UUID; member UUID;
BEGIN
    IF NEW.source_trace_id IS NULL OR NEW.audience_source_post_id IS NOT NULL THEN RETURN NEW; END IF;
    SELECT * INTO source FROM traces WHERE id=NEW.source_trace_id;
    IF source.trace_type <> 'USER_TRACE' THEN RETURN NEW; END IF;
    IF NEW.audience_role<>'DEFAULT' THEN
        DELETE FROM post_audiences WHERE post_id=NEW.id AND is_default;
    END IF;
    IF TG_OP='INSERT' AND NEW.user_id=source.user_id AND NEW.audience_role='DEFAULT'
        AND source.sharing_sensitivity='NORMAL' THEN
        INSERT INTO post_audiences(post_id,audience_id,is_default)
        SELECT NEW.id,d.audience_id,TRUE FROM journal_default_audiences d
        JOIN audiences a ON a.id=d.audience_id AND a.archived_at IS NULL AND a.owner_user_id=NEW.user_id
        JOIN journals j ON j.id=d.journal_id AND j.status='ACTIVE' AND NOT j.is_encrypted
        WHERE d.journal_id=source.journal_id ON CONFLICT DO NOTHING;
    END IF;
    IF NEW.status <> 'PUBLISHED' THEN RETURN NEW; END IF;
    IF TG_OP='UPDATE' AND OLD.status='PUBLISHED' THEN RETURN NEW; END IF;
    FOR aid IN SELECT audience_id FROM post_audiences WHERE post_id=NEW.id
        AND (NOT is_default OR (NEW.audience_role='DEFAULT' AND source.sharing_sensitivity='NORMAL'))
        ORDER BY audience_id LOOP
        PERFORM 1 FROM audiences WHERE id=aid FOR UPDATE;
        FOR member IN SELECT user_id FROM audience_members WHERE audience_id=aid
            AND status='ACTIVE' AND future_access_enabled ORDER BY user_id LOOP
            PERFORM grant_audience_post(aid,NEW.id,member,FALSE);
        END LOOP;
    END LOOP;
    RETURN NEW;
END;
$$;
