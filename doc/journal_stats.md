# Journal statistics

Standard authenticated journal responses now include a computed `stats` object:

```json
{
  "id": "journal-uuid",
  "stats": {
    "traces_count": 42,
    "oldest_trace_finalized_at": "2026-01-12T09:30:00Z",
    "newest_trace_finalized_at": "2026-11-01T08:00:00Z"
  }
}
```

Included on `GET /journals/:id`, `GET /users/:id/journals`,
`GET /journals/shared`, `GET /journals/shared/recent`,
`GET /users/:id/journals/shared`, and journal creation/update responses.
No new statistics route, request parameter, database field or migration is needed.

## Meaning

- Count finalized `USER_TRACE` records in the journal, excluding drafts, archived
  traces, complements, special internal trace types, versions and publications.
- A trace is counted once even if multiple posts expose it.
- Dates are the minimum/maximum `finalized_at`, not creation, editing or publication
  dates. Imported historical finalized dates are retained.
- Owners see all matching traces in their journal, including encrypted records;
  only counts and dates are calculated, never ciphertext lengths.
- Readers see only currently accessible non-encrypted traces, including active
  mentions and social grants. Blocks, sandbox boundaries and journal visibility
  are respected. A public blog publication does not grant private journal access.
- Empty selections return `traces_count: 0` and both dates `null`.
- The three aggregates are computed together in one SQL query for the entire page
  of journals; no extra request or query per journal is necessary.

`last_trace_at` remains the existing activity/sorting field. Use the new stats dates
for the finalized-trace date range. Paginated collection `total` still counts
journals, not traces. Public sharing-link responses and export formats are unchanged;
internal/unhydrated journal objects may omit `stats` rather than return fake zeros.

Potential later additions are `characters_count` (where plaintext is available),
`active_days_count` (finalization days in the owner's timezone), reader-specific
`unread_traces_count`, or `traces_with_images_count`. They are not included yet.
