# Journal images

```http
GET /journals/:id/images?limit=12&offset=0
```

Requires an authenticated session and access to the journal. Uses standard
pagination: `limit` defaults to 20, maximum 100; `offset` defaults to 0.

```json
{
  "items": [
    {
      "trace_id": "trace-uuid",
      "finalized_at": "2026-11-01T09:00:00Z",
      "image": {
        "id": "asset-uuid",
        "width": 1200,
        "height": 1600
      }
    }
  ],
  "total": 27,
  "limit": 12,
  "offset": 0
}
```

- One item per finalized `USER_TRACE` with a ready image in
  `content_image_asset_id`. No drafts, archived traces, complements, transcription
  source photos or document attachments.
- Order: `finalized_at DESC NULLS LAST, trace_id DESC`, regardless of when the
  image was uploaded. Legacy missing finalization dates are returned as `null`.
- Owners see all matching traces. Readers see only traces they can currently
  access through social publications or active mentions. Blocks, sandbox
  boundaries, encryption and journal visibility are respected. Blog publication
  alone does not grant private journal access.
- Several publications of a trace do not duplicate the result. If two traces use
  the same asset, each has its own item so navigation still targets that trace.
- `total` counts matching traces visible to the caller, not all journal images.
  An accessible journal with no matching images, or a page beyond the end,
  returns HTTP 200 with `items: []`. An inaccessible journal is rejected using
  the existing journal authorization convention.
- Dimensions can be `null` for older assets. The response does not include trace
  text, storage paths or signed URLs. Resolve the image through the existing
  authenticated `GET /assets/:id/url` endpoint, which checks access again.

No new persistence table, migration or external provider call is needed.
