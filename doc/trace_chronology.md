# Trace chronology

The authoritative trace dates are:

- `start_writing_at`: UTC timestamp, nullable while an ordinary draft is empty.
  Set by the backend on the first meaningful write. Clearing a draft back to
  empty clears this timestamp.
- `finalized_at`: UTC timestamp of completion, null for drafts. Normal
  finalization uses the server clock. Editing an already-finalized trace does
  not move it to another day.
- `created_at` / `updated_at`: technical persistence timestamps, not activity dates.

## Compatibility and clients

`interaction_date` is deprecated, not an independently editable date. Old
request bodies may still include it, but it is ignored on trace creation,
updates and transcription confirmation. No new client-editable start/completion
fields are introduced. Responses retain the legacy field as a compatibility
alias of `finalized_at`, falling back to `start_writing_at` and then `created_at`.
The database column remains temporarily for old queries/clients and is maintained
by a trigger. It can be dropped in a later API cleanup.

Clients should use `finalized_at` for finished writing and `start_writing_at`
for active drafts, and tolerate null writing-start times on empty drafts.

## Analysis and display

Trace ordering, navigation, search chronology, journal recency, exports,
incremental analysis progression, daily/weekly coverage and weekly day grouping
use completion chronology. Draft/context fallbacks use writing-start/creation.
Calendar boundaries remain user-local and stored timestamps remain UTC.
Extracted entities and analyses retain their own date attributes; they are not
globally renamed or removed by this change.

Finishing a trace still starts extraction immediately. A daily/weekly recap
cannot run before its completion-based period ends, and must contain at least
one ordinary non-imported finalized trace. Push is sent when feedback is ready;
this does not introduce a new morning push-delivery schedule.

## File imports and replays

Text/Markdown imports set both writing-start and completion from each parsed
header date. Date-only headers use noon in the user's timezone. Explicit ISO
offsets are respected. Older Hupo `export_generated_date` headers without an
offset retain their UTC interpretation; new journal exports include an offset.

Missing/invalid dates, missing years and ambiguous/nonexistent local times use
import time. No hardcoded year or previous-entry-plus-one-day date is invented.
The import response adds `date_fallback_block_indices` (zero-based block indices)
so this fallback is visible. Existing initial journal-introduction handling is
preserved.

File-imported traces are marked internally as historical imports. They can be
extracted for context, but do not independently schedule historical recaps and
notifications. This marker does not alter access or publication grants. Ordinary
photo transcription remains a draft, not a dated file import.

The local `import_txt` utility delegates to the same import service. Pipeline
playground exports/replays preserve stored writing-start/completion timestamps
and process traces in completion order; replay mode can still exercise historical
daily/weekly analysis without sending emails or push.

## Deployment

The migration keeps existing completion dates, clears empty-draft writing-start
markers, fills missing completion dates on old finalized records conservatively,
updates search metadata and retimes only unfinished incremental jobs. Completed
analyses and already-sent feedback are not rewritten or re-sent.

After deploying the new binary/migration, use the existing authenticated internal
`POST /internal/replan_autoplay_lenses` endpoint to reconcile pending recap
coverage/planning. This operation queues work without calling LLMs itself.
Completed recap inputs are left untouched. Old empty pending recap windows are
not eligible for execution. Then the usual internal scheduler processes due work.

Apply with the new backend; an old binary expects non-null writing-start values,
so a rollback to old code requires the migration rollback first. No production
backfill or pipeline run is implicitly performed by the implementation task.
