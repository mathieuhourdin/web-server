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
exposure, cooldowns and repetition/escalation state. Source/user deletion clears
state references through foreign keys. Accepted transitions additionally record
metadata-only history in the existing `usage_events` table; no content copies or
separate nudge-history table are introduced.

## Usage events and analytics

The existing event endpoint also records `WRITING_NUDGE_SHOWN`,
`WRITING_NUDGE_OPENED` and `WRITING_NUDGE_DISMISSED` usage events internally.
Clients must not submit these to `POST /usage_events` (400); keep using the nudge
endpoint only. Fetches and duplicate/stale/foreign events create no usage events.
An accepted opening/dismissal before exposure creates both SHOWN and the terminal
event. Tracking and cooldown updates commit atomically, including under concurrent
requests. The first terminal action still wins.

`resource_id` is the suggestion UUID. `context_json` contains only
`{"nudge_type":"resume_draft","trace_id":"uuid-or-null"}` (a JSON null for
first-trace invitations), never titles, excerpts or trace content. Event times
are server-assigned UTC. No session is required for the historical association:
the authenticated user ID is recorded.

Admin-only daily series:

`GET /admin/analytics/platform_series?from=2026-11-01&to=2026-11-30&metric=writing_nudge_shown&metric=writing_nudge_opened&metric=writing_nudge_dismissed&metric=writing_nudge_shown_users&metric=writing_nudge_opened_users&metric=writing_nudge_dismissed_users`

Each metric uses the existing `{metric,points:[{date,count}]}` envelope. The first
three count accepted suggestion events; the `_users` variants count distinct
users per event and day. Days follow each user's current timezone, with the
existing timezone-alias/UTC fallback. Zero days are included. Existing default
metric selections are unchanged; nudges do not count as reading or writing.

Admin-only exposure-cohort breakdown:

`GET /admin/analytics/writing_nudges?from=2026-11-01&to=2026-11-30`

```json
{
  "from": "2026-11-01",
  "to": "2026-11-30",
  "as_of": "2026-11-30T12:00:00Z",
  "types": [
    {
      "nudge_type": "first_trace",
      "shown": 10,
      "opened": 3,
      "dismissed": 2,
      "no_recorded_action": 5,
      "shown_users": 8,
      "opened_users": 3,
      "dismissed_users": 2,
      "opening_rate": 30.0,
      "dismissal_rate": 20.0
    }
  ]
}
```

All three types are always returned, including zero rows. The date range selects
suggestions first **shown** in that range (same local-day convention as daily
series); terminal actions for those suggestion IDs are followed through `as_of`,
even outside the range. Rates are percentages (0–100), divided by shown
suggestions of that type, or 0.0 when no exposures exist. Distinct-user counts are
per type and must not be added across types to obtain platform distinct users.
`no_recorded_action` is not proof that a suggestion was ignored; the card can
still be active. Opening is not evidence of completed writing.

Both analytics use human platform accounts and retained usage events; no stored
aggregates, background processing or backfill. History before instrumentation
cannot be reconstructed from the current nudge state. A card exposed before
deployment can therefore have a later terminal event but no historical SHOWN
event; it contributes to daily actions, not the exposure-cohort breakdown.
Date range defaults to the last 30 inclusive days, maximum 366 days, matching
existing admin analytics. Non-admin sessions are rejected with 403.

Deployment applies migration `2026-11-04-110000_writing_nudge_usage_events` at normal
startup. Rollback refuses to discard existing nudge events. No client event or
environment-variable changes are needed.
