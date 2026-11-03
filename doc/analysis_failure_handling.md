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

## Quota deferral and automatic catch-up

The landscape-analysis quota counts TRACE_INCREMENTAL, HLP and BIO analysis rows
created in a rolling 24-hour window (including pending/failed rows). Its limit is
30 per user, not 30 successful model calls. Daily/weekly recap rows are not counted.

Quota exhaustion during automatic planning is a pause, not a failed lens. Jobs
already created are retained and can run immediately. The lens stays OUT_OF_SYNC
while planning is incomplete, without setting a failure reason or consuming its
automatic failure retry. Genuine planning and processing errors still fail normally.

The existing run-pending cron also picks up OUT_OF_SYNC lenses with a target even
if no runnable jobs exist yet. Under the existing per-lens run lock, the worker
resumes idempotent planning before executing queued work. Once the rolling budget
has capacity, later passes create the remaining jobs. Planning and execution use
the same lock, so simultaneous finalization/cron planning does not create duplicate
jobs. AI-disabled accounts and platform users without a mentor remain excluded.
The internal replan endpoint reports quota/lock deferrals in `skipped` with reason
`planning_deferred`; a deferred pass may already have queued part of its batch.

A run becomes IN_SYNC only when planning completed and no eligible job remains;
future recaps can still wait for their period to end. A target changed while the
worker was running cannot be acknowledged by that old worker. Reaching the normal
100-analysis batch limit leaves the lens OUT_OF_SYNC for a later cron pass rather
than turning a large backlog into a processing failure. The playground's one-job
primitive still only executes one existing queued job, without resuming planning.

A failed lens schedules one automatic retry at failure time plus 24 hours if its
retry counter has not already been used. The internal run-pending cron activates
due retries and returns existing failed scoped analyses to PENDING. This is not a
midnight reset or a recurring daily retry. After activation the normal worker now
also resumes incomplete planning. Existing FAILED lenses are not rewritten or
immediately reopened by this change; they keep their scheduled/manual recovery.
No new migration, worker, cron job or environment variable is required.
