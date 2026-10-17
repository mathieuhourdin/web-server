DROP TRIGGER zz_trace_language_timestamp ON traces;
DROP FUNCTION preserve_trace_language_timestamp();
ALTER TABLE traces DROP COLUMN language_code;
