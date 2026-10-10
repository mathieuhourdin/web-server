-- Keep migrated rows and their IDs as evidence, not as live sharing state.
DROP TRIGGER mirror_journal_policy_audience ON journal_sharing_policies;
DROP TRIGGER mirror_audience_member_policy ON audience_members;

CREATE FUNCTION protect_retired_journal_policies() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
    -- Retention must not prevent account/journal deletion through FK cascades.
    IF TG_OP='DELETE' THEN
        IF NOT EXISTS (SELECT 1 FROM users WHERE id=OLD.owner_user_id)
            OR NOT EXISTS (SELECT 1 FROM users WHERE id=OLD.grantee_user_id)
            OR NOT EXISTS (SELECT 1 FROM journals WHERE id=OLD.journal_id) THEN
            RETURN OLD;
        END IF;
    END IF;
    RAISE EXCEPTION 'Journal sharing policies are retired; use audiences' USING ERRCODE='55000';
END;
$$;
CREATE TRIGGER protect_retired_journal_policies BEFORE INSERT OR UPDATE OR DELETE ON journal_sharing_policies
FOR EACH ROW EXECUTE FUNCTION protect_retired_journal_policies();
