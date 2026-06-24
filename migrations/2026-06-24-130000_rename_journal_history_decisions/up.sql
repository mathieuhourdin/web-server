ALTER TABLE journal_sharing_policies
DROP CONSTRAINT IF EXISTS journal_sharing_policies_history_decision_check;

UPDATE journal_sharing_policies
SET history_decision = CASE history_decision
    WHEN 'ALL' THEN 'ALL_INCLUDING_SENSITIVE'
    WHEN 'NORMAL_ONLY' THEN 'ALL_NORMAL'
    WHEN 'SPECIFIC' THEN 'USER_SELECTED'
    ELSE history_decision
END
WHERE history_decision IN ('ALL', 'NORMAL_ONLY', 'SPECIFIC');

ALTER TABLE journal_sharing_policies
ADD CONSTRAINT journal_sharing_policies_history_decision_check CHECK (
    history_decision IS NULL
    OR history_decision IN ('NONE', 'ALL_NORMAL', 'ALL_INCLUDING_SENSITIVE', 'USER_SELECTED')
);
