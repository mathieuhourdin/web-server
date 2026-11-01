# Analysis eligibility and failure recovery

Automatic trace finalization and analysis planning skip accounts unless both
`ai_features_enabled` and `ai_features_enabled_by_admin` are true. This happens
before creating autoplay work, reading trace content for planning, or checking
analysis quotas. Explicit AI-request routes still reject disabled AI with 403.
The execution queue already excludes disabled accounts. Next-day retry activation
also excludes them, so opting out does not consume the lens's recovery attempt.

Migration `2026-11-01-150000_clear_ai_disabled_analysis_failures` repairs only
FAILED rows whose stored reason exactly equals
`AI features are disabled for this account`:

- Lenses become OUT_OF_SYNC, with the false failure and retry bookkeeping cleared.
- Affected analyses become PENDING, with their false failure cleared.
- Other failures, including missing reasons, quota failures, overlaps and provider
  errors, are unchanged. No content, completed result or AI preference is changed.

This repair does not execute analyses. Pending work remains dormant while AI is
disabled. The down migration intentionally does not recreate the false failures.

## Current quota recovery limitations

The landscape-analysis quota counts TRACE_INCREMENTAL, HLP and BIO analysis rows
created in a rolling 24-hour window (including pending/failed rows). Its limit is
five, not five successful model calls. Daily/weekly recap rows are not counted.

A failed lens schedules one automatic retry at failure time plus 24 hours if its
retry counter has not already been used. The internal run-pending cron activates
due retries and returns existing failed scoped analyses to PENDING. This is not a
midnight reset or a recurring daily retry. Activation does not re-plan jobs that
were never created because planning stopped at the quota, so it does not guarantee
complete backlog recovery. That behavior is not changed by this eligibility fix.
