# Reader access, plumes and WAL scheduling

## Reader access audit

`GET /me/audience/:user_id/traces?limit=20&offset=0`

Authenticated owner-facing audit: returns only the caller's finalized ordinary
traces that the specified user can currently read, across all journals. Uses
effective publication access and active mentions, respecting blocks, encryption
and demo isolation. The target need not still be a follower.

Standard paginated envelope: `items` (trace objects), `total`, `limit`, `offset`.
Sorted by `finalized_at` descending, with a stable ID tie-breaker. `total` remains
accurate on empty/out-of-range pages. Personal metadata is for the caller, never
the inspected reader. The existing journal-specific endpoint is unchanged.

## Plumes

- `PUT /traces/:id/plume`: give a plume, no body. Requires current read access to
  someone else's finalized trace. Repeating the request does not add another.
- `DELETE /traces/:id/plume`: retract your own plume. Remains possible after
  losing access; in that case the response does not reveal the current count.
- `GET /traces/:id/plumes?limit=20&offset=0`: author-only paginated list of public
  user profiles, newest plume first. Counts remain correct beyond the last page.

PUT and DELETE return `{ "plume_count": 3, "plumed_by_me": true }` (DELETE returns
false). These same two fields are included on authenticated trace reads, journal
and search collections, feeds, album trace items and writing-thread traces.
Hydration is batched, not a query per trace. Non-trace feed cards use zero/false.
Plumes belong to the source trace, independently of its posts. They grant no
access; revocation does not erase a past plume. No plume notifications yet.

## Explicit audience membership only

Accepting a follow or creating/updating a shared journal no longer creates an
audience or adds members to one. Existing audiences, members, grants and pending
reviews are untouched. Journal default audiences remain supported, but their
membership is managed explicitly through the audience API. No new opt-in
auto-enrollment option is introduced.

## WAL debounce

Automatic compilation is due five minutes after the latest appended entry.
Each append, including carryover application, pushes the deadline back by five
minutes. The existing server worker executes due work with database claims,
leases, revision checks and AI-consent guards. This is an inactivity debounce,
not a periodic call every five minutes; it incurs no compilation call per save.
Manual POST /me/wal/compilation still requests immediate compilation.

GET/POST /me/wal and WAL collection objects now include `automation`, also
present in compilation responses:

```json
{"status":"scheduled","input_revision":12,"scheduled_for":"2026-10-19T10:35:00",
 "processing_revision":null,"started_at":null,"last_error":null}
```

Timestamps follow existing UTC timestamp conventions. `scheduled_for` is the
earliest eligible time, not a guaranteed start time. Append responses contain the
rescheduled deadline. Previous compilations stay visible while waiting; structured
views expose `is_stale: true` and their `source_revision`. Clients should not clear
these views or trigger automatic compilation on every edit.

## Email links

New publication, mention and daily/weekly reading-recap emails use the common
web/mobile `/traces/:id` link. Access checks still apply when opened. Existing
queued emails are not rewritten; conversation/journal-access links are unchanged.
