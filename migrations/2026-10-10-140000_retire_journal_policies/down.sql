DROP TRIGGER protect_retired_journal_policies ON journal_sharing_policies;
DROP FUNCTION protect_retired_journal_policies();
-- This restores the bridge, not a rollback of audience changes made since retirement.
CREATE TRIGGER mirror_journal_policy_audience AFTER INSERT OR UPDATE ON journal_sharing_policies
FOR EACH ROW EXECUTE FUNCTION mirror_journal_policy_audience();
CREATE TRIGGER mirror_audience_member_policy AFTER UPDATE ON audience_members
FOR EACH ROW EXECUTE FUNCTION mirror_audience_member_policy();
