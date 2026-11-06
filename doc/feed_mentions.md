# Mentions in feed cards

Authenticated `GET /feed`, its existing `GET /posts/feed` alias, and
`GET /me/feed` return `mentions` directly on every feed item, at the same level
as `source_id`, `content` and `language_code`.

```json
{
  "source_kind": "trace",
  "source_id": "trace-uuid",
  "mentions": [
    {
      "user_id": "mentioned-user-uuid",
      "handle": "@example",
      "display_name": "Example",
      "profile_picture_display_url": "https://example.invalid/avatar.jpg",
      "allows_reshare": false
    }
  ]
}
```

Other fields in the example are omitted for brevity. The mention item shape is
the existing `TraceMentionUser` contract used by trace detail/journal responses.
Display names respect pseudonymization. Profile URLs prefer an available public
profile asset and fall back to the legacy URL, or null when absent.

- Trace cards include only active mentions on their source trace. Removed mentions
  are excluded; traces without active mentions return `[]`.
- Document and album cards return `mentions: []`. Album cards do not aggregate
  mentions from child traces.
- The same trace appearing through multiple posts retains its mentions on every
  card. Use `post_id` as the card key when present; otherwise use
  `(source_kind, source_id)`, not a globally unique source ID across record types.
- Mention metadata is loaded only for traces in the already authorized, paginated
  feed page. This adds no new access grants and does not change feed visibility,
  totals, order, seen state, or notification behavior.
- Mentions, users and profile-asset URLs are loaded in batches; no per-card request
  or per-mention asset query is needed. No schema migration or new parameter.
