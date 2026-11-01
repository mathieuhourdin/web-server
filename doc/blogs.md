# Blogs and publication channels

## Model and privacy

A user can own multiple blogs. A blog ID is its publication channel ID. `slug`
is its unique lowercase URL identity, `name` its displayed title, and `username`
its independent public author name/pseudonym (not unique, not the account handle).
Renaming a blog does not change its slug; explicitly changing a slug changes its
URLs, without redirects in this first version.

A post belongs to one channel, or has no channel for ordinary social sharing.
There is one post per source/channel. The same source can be published to multiple
blogs with separate publication states. Blog posts have no social grants,
audiences, feed entries, push notifications or email notifications. Publishing
does not make the full authenticated trace API anonymously readable.

Publication is **live**: subsequent trace/album edits immediately affect the blog.
Only owned finalized, non-encrypted `USER_TRACE` traces and owned non-archived
albums may be published. Sensitive traces can be deliberately published too;
clients should clearly confirm that the selected content will be public.
Reshares, complements, documents and demo accounts are excluded in this version.
Archiving/encrypting a source hides it; restoring it does not automatically
republish an archived post. An album subject only lists traces with their own
published post on that same blog. Adding private album items never publishes them.
The album's title, subtitle, content and selected cover image are public.

## Management API (authenticated owner only)

`GET /blogs?limit=20&offset=0` returns the caller's blogs in the standard
`{items, total, limit, offset}` envelope, newest first.

`POST /blogs` creates a blog (201). `PUT /blogs/:id` fully replaces its settings:

```json
{
  "slug": "notes-sur-le-travail",
  "name": "Notes sur le travail",
  "username": "Le Promeneur",
  "description": "Un carnet public.",
  "image_asset_id": null,
  "enabled": false
}
```

`slug`: 1–80 ASCII lowercase letters/digits with single hyphens between words;
reserved routing names are rejected. `name`: 1–160 characters. `username`:
1–120 characters. `description`: at most 4000 characters, defaults to empty.
`enabled` defaults to false; omitted `image_asset_id` clears the image on PUT.
Images must be owned, ready JPEG/PNG/WebP/GIF assets, uploaded through the existing
asset upload API. Unknown settings fields are rejected.

Responses contain these settings plus `id`, `created_at`, `updated_at`.
The account owner ID and real account name are not exposed in the public pages.

- `GET /blogs/:id`: settings, including disabled blogs.
- `DELETE /blogs/:id`: deletes this blog/channel and its publications (204),
  never the original traces, albums, assets or social posts.
- `PUT /blogs/:id/traces/:trace_id`: publish/republish a trace, no request body.
- `PUT /blogs/:id/albums/:album_id`: publish/republish an album subject, no body.
- `GET /blogs/:id/posts?limit=20&offset=0`: all its publications, including archived
  ones, with pagination. Descending publication date, then ID.
- `GET /blogs/:id/posts/:post_id`: one publication.
- `DELETE /blogs/:id/posts/:post_id`: unpublish (archive), 204.

Repeated publishing reuses the post ID and original publication date. Explicit
republishing events use `POST /posts/:id/publications` with notification mode
`none`; their last timestamp determines article ordering. See
`GET /api/docs?topic=post_publications` for archival snapshots and event history.
Publishing
to a disabled blog is allowed for preparation but nothing becomes anonymously
visible until the blog is enabled.

Publication responses use this narrow live projection:

```json
{
  "id": "uuid",
  "publication_channel_id": "uuid",
  "source_trace_id": "uuid-or-null",
  "source_album_id": null,
  "status": "PUBLISHED",
  "publishing_date": "2026-10-31T12:00:00",
  "last_publication_id": "uuid",
  "last_publication_at": "2026-10-31T12:00:00",
  "title": "Article",
  "subtitle": "",
  "content": "Current source content",
  "image_asset_id": null,
  "finalized_at": "2026-10-30T12:00:00"
}
```

## Anonymous HTML and media

- `GET /blog/:slug?limit=20&offset=0`: paginated public blog page.
- `GET /blog/:slug/posts/:post_id`: article or subject page. For subjects,
  pagination applies to visible articles; order follows the album's manual,
  added-at or chronological order (chronological uses finalized_at).
- `GET /blog/:slug/assets/:asset_id`: selected blog image only.
- `GET /blog/:slug/posts/:post_id/assets/:asset_id`: selected article/subject image only.

Disabled blogs, unpublished articles and unavailable sources return 404. HTML
escapes all source text and includes canonical/Open Graph metadata, a restrictive
CSP and `Cache-Control: no-store`. Text is displayed as escaped text preserving
paragraph breaks, not interpreted as arbitrary HTML. No client-side JavaScript.

Storage stays private. Media routes check the current public publication and exact
asset association before redirecting to a 60-second signed URL. Previously issued
URLs can remain valid for that interval. Readers can always retain downloaded
content; unpublishing cannot erase external copies/search engine caches.
Original diary photos, documents, private album items, mentions, complements,
mentor messages and other linked data are never hydrated into public pages.

## Deployment (separate from implementing this feature)

Set `BLOG_BASE_URL=https://blog.hupo.fr` for absolute page/media/canonical URLs.
Without it, local URLs use `/blog`. Add DNS and TLS for `blog.hupo.fr`, and configure
nginx to proxy the public hostname to the Axum `/blog/` prefix. A sample is in
`ops/blog/nginx.conf.example`; do not replace the existing API server block.
Normal startup applies the new migration. No new bucket, public bucket policy,
LLM credentials or worker is needed.

## Internal social/channel boundary

All publications live in the physical `posts` table, mapped directly by Diesel's
`schema::posts`. Social queries and mutations explicitly require
`publication_channel_id IS NULL`; channel code explicitly selects its destination.
There is no separate social-posts table, SQL view, or hidden name mapping.
Database access functions exclude channel posts, and guards reject social
grants/audiences on channel posts and enforce channel ownership.
Source archival/encryption cascades still operate on all publications.

No public blog data is migrated from existing social posts automatically. The down
migration deletes channel publications and blogs, leaving social content intact.
