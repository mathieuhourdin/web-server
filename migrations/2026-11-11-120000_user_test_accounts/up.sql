ALTER TABLE users ADD COLUMN is_test_account boolean NOT NULL DEFAULT false;

-- One live population for product analytics, not a materialized aggregate.
-- Admin management and per-user diagnostics continue to query users directly.
CREATE VIEW analytics_users AS
SELECT u.id,u.created_at,u.timezone,u.principal_type,u.is_platform_user,
    u.onboarding_version,u.mentor_id,u.current_lens_id,u.pseudonymized,
    u.pseudonym,u.first_name,u.last_name
FROM users u
WHERE NOT u.is_test_account
  AND NOT EXISTS(SELECT 1 FROM automation_demo_users d WHERE d.user_id=u.id);
