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
- `GET /blogs/:id/publications?limit=20&offset=0`: all its publications, including archived
  ones, with pagination. Descending publication date, then ID.
- `GET /blogs/:id/publications/:post_id`: one publication.
- `DELETE /blogs/:id/publications/:post_id`: unpublish (archive), 204.

### Publications across all of my blogs

```http
GET /users/:id/publications?limit=20&offset=0
```

Authenticated owner only: `:id` must be the caller's user ID. This does not expose
the links between an account and its blogs/pseudonyms to other users.

Uses the standard `{items, total, limit, offset}` envelope (default limit 20,
maximum 100). Each item contains the existing blog-post fields shown below, plus:

```json
{
  "id": "post-uuid",
  "publication_channel_id": "blog-uuid",
  "source_trace_id": "trace-uuid-or-null",
  "source_album_id": null,
  "status": "PUBLISHED",
  "blog": {
    "id": "blog-uuid",
    "slug": "notes-sur-le-travail",
    "name": "Notes sur le travail",
    "username": "Le Promeneur",
    "enabled": true
  }
}
```

- Includes `PUBLISHED` trace posts and album-subject posts across every owned blog.
- Includes posts on disabled blogs for owner management; `PUBLISHED` alone does
  not imply that a page is currently anonymously available.
- Excludes internal/social posts, archived/unpublished posts and other users'
  posts. The existing per-blog management collection still includes archived posts.
- One row per blog post, not per publication event. The same trace published on
  two blogs appears twice, with different post/channel IDs.
- Sorted by `last_distribution_at DESC NULLS LAST, id DESC`, not trace creation or
  finalization date. Explicit republication updates the existing row's position.
- `total` counts matching posts even for an empty/out-of-range page. Empty
  collections return 200 with `items: []` and `total: 0`.
- Reuses the narrow live post projection; does not hydrate journals, mentions,
  translations, source photos or private album items. Blog metadata is joined
  in the paginated query, not fetched separately for each post.

This is a durable publication collection, not the `post_distribution_events` event ledger. No new
table, migration or client parameter is needed beyond normal pagination.

Repeated publishing reuses the publication's post ID and original publication date. Explicit
redistribution events use `POST /posts/:id/distribution_events` with notification mode
`none`; their last timestamp determines article ordering. See
`GET /api/docs?topic=post_distribution_events` for archival snapshots and event history.
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
  "last_distribution_event_id": "uuid",
  "last_distribution_at": "2026-10-31T12:00:00",
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

Disabled blogs, unpublished articles and unavailable sources return 404. Public
article/subject bodies render CommonMark Markdown server-side, with tables and
strikethrough supported. Paragraphs, headings, lists, quotes, links, images and
code blocks are styled for reading. Ordinary single line breaks wrap naturally;
blank lines create paragraphs, and Markdown hard breaks remain supported.

Raw HTML is escaped, and generated HTML is sanitized; unsafe URL schemes such as
`javascript:` and `data:` are removed. HTTP(S), email and relative links are
supported. Titles, subtitles, blog descriptions and metadata remain plain escaped
text. List excerpts are plain text extracted from Markdown, without formatting
syntax or link destinations. Source content and API JSON remain unchanged.
Canonical/Open Graph metadata, the restrictive CSP and `Cache-Control: no-store`
remain in place. No client-side JavaScript, migration or new environment variable.

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
