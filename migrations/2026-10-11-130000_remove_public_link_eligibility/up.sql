-- Public journal links now use trace sensitivity, not historical audience scope.
-- Existing normal traces are no longer excluded by an old restricted-post flag.
ALTER TABLE posts DROP COLUMN public_link_eligible;
