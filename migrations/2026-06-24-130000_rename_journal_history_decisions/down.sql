ALTER TABLE journal_sharing_policies
DROP CONSTRAINT IF EXISTS journal_sharing_policies_history_decision_check;

UPDATE journal_sharing_policies
SET history_decision = CASE history_decision
    WHEN 'ALL_INCLUDING_SENSITIVE' THEN 'ALL'
    WHEN 'ALL_NORMAL' THEN 'NORMAL_ONLY'
    WHEN 'USER_SELECTED' THEN 'SPECIFIC'
    ELSE history_decision
END
WHERE history_decision IN ('ALL_INCLUDING_SENSITIVE', 'ALL_NORMAL', 'USER_SELECTED');

ALTER TABLE journal_sharing_policies
ADD CONSTRAINT journal_sharing_policies_history_decision_check CHECK (
    history_decision IS NULL
    OR history_decision IN ('NONE', 'ALL', 'NORMAL_ONLY', 'SPECIFIC')
);
