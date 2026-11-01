# Album statistics

Standard authenticated album objects include:

```json
{
  "id": "album-uuid",
  "stats": {
    "traces_count": 12,
    "journal_ids": ["journal-uuid-1", "journal-uuid-2"]
  }
}
```

Returned by `GET /albums/:id` (alongside `items`), `GET /users/:id/albums`,
`GET /traces/:id/albums`, album creation/update and cover-image upload responses.
The collection envelope's `total` continues to count albums, not traces.

## Viewer-dependent values, same response shape

- **Owner:** count every trace currently in the album, matching the owner's item
  collection. This includes private items, and drafts/archived items if present;
  unlike journal stats, this is not a finalized-trace-only count.
- **Reader:** count only currently visible album items through eligible social
  publications. Existing album visibility, trace finalization/encryption, active
  grants, blocks, resharing authorization and sandbox checks are retained. Blog
  publication alone does not count as private album access. A mention alone does
  not expand the existing album visibility rules.
- `journal_ids` contains the distinct non-null journal IDs of **that same set**
  of traces, sorted by UUID. A reader never receives IDs of journals represented
  only by hidden items. Journal names or additional access are not returned.
- Several posts exposing the same trace do not multiply its count.
- Empty selections return `{"traces_count":0,"journal_ids":[]}`. Inaccessible
  albums are still rejected/omitted by the normal album routes, not exposed as
  empty public album objects.

Stats work for manual and chronological albums. The existing
`visible_trace_date_range` remains unchanged: it uses `finalized_at` and is only
populated for chronological albums with visible dated traces.

Both statistics and the date range are computed in **one grouped SQL query per
album page**, without loading trace content or issuing a query per album/item.
No persisted counter, migration or query parameter is added. Internal/unhydrated
album objects can omit `stats`; omission means not calculated, not zero.

Feed cards and public blog projections retain their existing contracts; this
change adds statistics to standard album resource responses only. Clients should
not reuse/cache these statistics between different signed-in users.
