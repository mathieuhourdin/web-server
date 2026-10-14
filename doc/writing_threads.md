# Writing threads — owner-only MVP

Threads are explicit single chains. Traces store a `continues_trace_id`.
A thread stores its start, latest finalized trace and an internal pending-draft
pointer. A branch starts another thread at the selected source trace; its analysis
uses that trace and its descendants, not the earlier prefix of another thread.
There is no trace–thread membership table and no access inheritance. All routes require
authentication and are owner-only; other users get 404. Ordinary trace reads hide
continuation IDs from nonowners.

## Link a draft

`PATCH /traces/:id` or `PUT /traces/:id` (also supported by the journal draft PATCH):

```json
{
  "continues_trace_id": "source-trace-uuid",
  "expected_version_integer": 4
}
```

Returns the updated Trace with its incremented version. Missing `continues_trace_id`
leaves the link unchanged; explicit `null` removes it. Links can be assigned,
removed or changed while the target is a draft. A finalized trace without a link
may also receive its first link; existing finalized links remain immutable.
Content and linking can be
saved together, and PUT may link and finalize in the same request.

The frontend chooses whether to reuse a draft, finish it or create another.
Existing nonempty drafts may be linked: the server never replaces or clears them.
The source must be an owned, finalized, unencrypted ordinary trace in the same
active, unencrypted user journal. Linking never copies content or sharing rights.

Thread bookkeeping is automatic: continuing a thread's latest trace extends it;
continuing an earlier trace starts a separate branch. If a continuation is already
pending, another continuation starts a separate thread. Relinking/unlinking releases
the old pending association and removes an abandoned one-trace thread shell.

Linking the beginning of an existing chain extends that thread backward, retaining
its ID. Connecting it to an upstream thread merges the two records, retaining the
downstream ID and regenerating its projection. Actual branches remain separate;
additional branches starting at the join keep their own starting point. An upstream
thread with a pending draft is not absorbed, since that draft is another continuation.

There are no POST /writing_threads or POST /writing_threads/:id/continue routes.

Save/finalize through the existing trace/draft endpoints with optimistic versions.
Finalization advances the thread and clears its pending pointer automatically, including
timeout finalization. Draft edits do not bump thread activity or regenerate analyses.
`latest_trace_id` is the stable last-appended finalized trace (the chain end), even
if it is subsequently archived. Archiving does not move this pointer or lose the
tail, so restoring a trace restores its place in the thread. An archived/encrypted
end sets `can_continue` false; branch from an earlier eligible trace instead.

## Browse and search

- `GET /writing_threads?journal_id=uuid&q=reflexion&limit=20&offset=0`
- `GET /journals/:id/writing_threads?q=reflexion&limit=20&offset=0`
- `GET /traces/:id/writing_threads?limit=20&offset=0`
- `GET /writing_threads/:id`
- `GET /writing_threads/:id/traces?limit=20&offset=0`

Collections use the existing paginated response shape. Thread traces are ordered
from start to end; archived traces are traversed but not returned. The active draft
is separate, available through `pending_trace_id` and the existing draft route.

Thread collections sort by latest finalization descending, with stable ties. This is
a recency baseline, not an AI relevance score. Search is accent-insensitive, against
generated titles, ongoing questions, journey summaries/tags and the latest trace title. No contextual ranking or
embedding is implemented. Start-at-one-trace threads are returned too.

A ThreadView contains:

```json
{
  "id": "uuid",
  "user_id": "uuid",
  "start_trace_id": "uuid",
  "latest_trace_id": "uuid",
  "pending_trace_id": "uuid or null",
  "journal_id": "uuid",
  "latest_trace_title": "The latest trace title",
  "last_activity_at": "2026-10-13T12:00:00",
  "trace_count": 2,
  "can_continue": true,
  "title": null,
  "ongoing_question": null,
  "journey_summary": null,
  "search_tags": [],
  "input_revision": 2,
  "compiled_revision": null,
  "generation_status": "PENDING",
  "generation_error": null,
  "generated_at": null,
  "created_at": "2026-10-13T11:00:00",
  "updated_at": "2026-10-13T12:00:00"
}
```

## AI generation

The server starts a persisted-queue worker alongside the WAL workers. Creation,
finalization and edits/archive/encryption of compiled-chain traces schedule generation
after a 15-second debounce, checked every 10 seconds, with two concurrent jobs.
Claims have a 10-minute lease and an ID/revision fence: changed inputs and reclaimed
jobs cannot overwrite newer projections. A restart resumes pending/expired work.

The dedicated prompt generates:

- `title`: a short, recognizable title intended to stay relatively stable.
- `ongoing_question`: a concise living question/tension/intuition, or null when none is supported.
- `journey_summary`: a fluid account of the movement of thought, typically 80–140 words
  (shorter for sparse material), without forcing progress or psychological interpretations.
- `search_tags`: grounded search phrases, as before.

All four fields are returned by single-item and collection routes; title and
journey_summary are nullable until generation succeeds. There is no longer a
description field in the API. Search indexes all four fields, with higher index
weights for title/tags; results remain ordered by recency.
The old database description column is retained only for rollback, not used or exposed.
Existing projections are queued for regeneration during migration. This is not a
mentor feedback or an exhaustive summary. It uses the existing `gpt-5.6-luna`
configuration and records LLM tokens/cost estimates through the user-associated handler.
Existing AI consent/admin enablement and automation quotas apply, plus a 20-request
rolling 24-hour per-user limit. Empty or encrypted chains are not submitted. Sources
larger than 120,000 characters fail explicitly rather than silently losing content.

Statuses: PENDING, RUNNING, READY, FAILED, DISABLED. Content changes immediately clear
the old projection/search index, including encryption or archival changes. The client
can use `latest_trace_title` as fallback while generation is unavailable.

`POST /writing_threads/:id/generation` queues a stale/failed/disabled projection after
AI is enabled; it does not wait for the LLM or repeat already-current generation.
Fetch the thread to observe completion. Failures require this explicit retry or a new
content change; expired worker leases are recovered automatically.

Cross-journal chains, changing/removing existing finalized links, reader thread navigation, AI
discovered links and membership materialization are deliberately not in this version.
