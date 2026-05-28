ALTER TABLE traces
ADD COLUMN sharing_sensitivity TEXT NOT NULL DEFAULT 'NORMAL';

ALTER TABLE traces
ADD CONSTRAINT traces_sharing_sensitivity_check
CHECK (sharing_sensitivity IN ('NORMAL', 'SENSITIVE'));

ALTER TABLE trace_versions
ADD COLUMN sharing_sensitivity TEXT NOT NULL DEFAULT 'NORMAL';

ALTER TABLE trace_versions
ADD CONSTRAINT trace_versions_sharing_sensitivity_check
CHECK (sharing_sensitivity IN ('NORMAL', 'SENSITIVE'));
