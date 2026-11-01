-- Keep all archival data: only remove future automatic capture behavior.
DROP TRIGGER archive_finalized_trace_version ON traces;
DROP FUNCTION archive_finalized_trace_version();
