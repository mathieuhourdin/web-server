# Own-feed sharing filters and selected-journal exports

Both routes require the authenticated account owner. Existing response shapes are unchanged.

## Own feed

```
GET /me/feed?sharing=shared&limit=20&offset=0
GET /me/feed?sharing=private&limit=20&offset=0
```

Omit `sharing` for the existing combined feed, including albums. Supplying either
value returns only finalized ordinary traces; album cards are excluded. Invalid
values return 400. Filtering precedes sorting, counting and pagination.

`shared` means the trace is currently exposed through at least one of:

- A published social post with an active readable grant to another user, or an
  eligible active platform-wide grant. Secondary publishers are included.
- An active mention of another user, respecting blocks and demo isolation.
- A published trace on an enabled public blog.
- An unexpired, unrevoked sharing link that currently exposes this trace.
  Whole-journal links exclude sensitive traces; explicit single-trace links do not.

`private` is the complement of this condition. Encrypted traces are private.
A prepared or published social post with no readers is not sufficient to make
a trace shared. Removing the last effective sharing path makes it private again.
This describes current exposure, not whether a trace has ever been shared or read.

The existing `status` field still describes the social post status, not this
sharing classification; a trace shared only on a blog can have a draft social post.
Sorting, batched hydration and the paginated envelope are unchanged. No new per-card
requests or persisted sharing flags are introduced.

## Export selected journals

```
GET /me/traces/export?format=md&journal_ids=uuid1,uuid2&from=2026-01-01&to=2026-12-31
```

- `format`: required `md`, `txt` or `json`, unchanged.
- `journal_ids`: optional comma-separated UUIDs, at most 100. Duplicates are
  deduplicated. An empty selection or malformed UUID returns 400. Any unknown or
  non-owned journal returns 404; no partial export is produced.
- Omission exports all owned journals, unchanged.
- `from` and `to`: existing optional inclusive calendar-date bounds.

The response remains a downloadable file, chronologically sorted by finalized
date (writing-start/creation fallback for unfinished traces). Existing draft,
archive and blank-placeholder behavior is unchanged. JSON exports contain only
the selected journals in `journals`, and only their traces in `traces`; the JSON
schema version and shape are unchanged. Empty selected journals are retained in
the JSON journal metadata.
