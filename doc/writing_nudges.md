# Contextual writing nudges

Nudges are private, on-demand suggestions for the authenticated user. They are
independent of the feed, pagination, unread counts, trace-seen tracking and AI
consent. There are no LLM calls, notifications, workers or scheduled jobs.

## Fetch

`GET /me/writing_nudges/current` returns 200 with `{ "nudge": null }` or:

```json
{
  "nudge": {
    "id": "11111111-1111-4111-8111-111111111111",
    "type": "resume_draft",
    "issued_at": "2026-11-03T10:00:00Z",
    "expires_at": "2026-11-03T23:00:00Z",
    "trace": {
      "id": "22222222-2222-4222-8222-222222222222",
      "journal_id": "33333333-3333-4333-8333-333333333333",
      "title": "Quelques idees",
      "excerpt": "Une question a reprendre…",
      "finalized_at": null,
      "updated_at": "2026-10-29T16:00:00Z"
    }
  }
}
```

Types are `resume_draft`, `writing_memory`, `first_trace`. `trace` is null only
for `first_trace`. `journal_id` and `finalized_at` are nullable; title/excerpt
can be empty. Excerpts are plain text, at most 240 Unicode characters. All
timestamps are UTC RFC 3339. Previews are not editable snapshots: load the
canonical trace before entering the editor. Copy, labels, template selection
and navigation remain client-side.

Fetching can reserve a stable suggestion but does not record exposure or start
a cooldown. Repeated requests return the same valid ID until midnight in the
user's timezone, a terminal event, supersession or loss of eligibility. Invalid
timezones fall back to UTC; midnight gaps/ambiguities are resolved server-side.
Responses use `Cache-Control: private, no-store`.

## Events

`POST /me/writing_nudges/:nudge_id/events`, JSON `{ "event": "shown" }`.

Accepted values: `shown`, `dismissed`, `opened`. Successful requests return 204
with no body. Unknown, stale, expired, superseded or foreign IDs are no-op 204s.
Only the caller's state can change. Repeated exposure/terminal events are
idempotent and do not extend cooldowns; the first terminal event wins. A terminal
event arriving before `shown` implicitly records exposure. Timestamps are
server-assigned. These calls never modify/finalize traces or mark them as seen.

Missing authentication returns 401. Malformed JSON/UUIDs return 400, missing or
unsupported event values return 422, and incorrect content type returns 415.
`opened` means the destination opened, not that writing was completed.

## Selection and timing

Priority: unfinished draft, writing memory, first-writing invitation.

- Draft: meaningful ordinary writing, inactive at least 3 days; choose the most
  recently edited eligible draft. Expired timeout drafts are not suggested.
- Memory: some prior writing and at least 14 days without activity. Prefer the
  most recent eligible trace at least 30 days old; fall back to more recent
  finalized writing. Activity includes recent imports so importing old writing
  does not immediately trigger an inactivity suggestion.
- First: no existing meaningful ordinary writing. Blank auto-created drafts
  do not count; archived and encrypted writing does count as writing history.

Candidates are owned, non-blank USER_TRACE/WORKSPACE_TRACE records. They are
never archived/encrypted, or from another user's/archived/encrypted journal.
Only source IDs/timestamps and the display-safe preview are returned. Archived
and encrypted records are considered through metadata only, never as previews.

Trace `updated_at` is the MVP's conservative edit-activity proxy, combined with
finalization/creation timestamps. Metadata-only updates may postpone a nudge.
No new content-edit timestamp or writing-success event is introduced.

- One distinct suggestion exposed per local day, globally across surfaces and
  devices. The same active card can remain visible on that day.
- Exposure or opening pauses new suggestions for 3 days; opening can extend the
  pause from the opening time. Fetching alone never starts a pause.
- Dismissal pauses all writing nudges for 7, then 14, then 30 days, based on
  repeated dismissals of that type. Subsequent meaningful non-import writing
  resets future dismissal escalation, not an already-promised quiet period.
- Avoid consecutive reuse of an exposed source trace for that type. If it is
  the only candidate, allow reuse after 30 days. This is not a full per-trace
  history or a guarantee against alternating between two traces.

## Mobile

Fetch alongside the feed or on entering another supported surface, not on each
pagination request. Share current state across screens. Report `shown` once at
least 50% of the card was visible continuously for one second. Hide immediately
after opening/dismissal; invalidate eligibility after writing or importing. Check
expiration when displaying cached state or resuming the app. Fetch/tracking errors
must not block navigation, writing or ordinary feed rendering.

## Persistence

`user_writing_nudge_states` stores at most three rows per user, one per type.
User-row locking serializes selection/tracking across all types and devices.
Current suggestion timestamps/terminal state are separate from aggregate last
exposure, cooldowns and repetition/escalation state. No append-only event table
or content copies are kept; source/user deletion clears references/state through
foreign keys. No unrelated trace mutations or usage-event types are added.
