# Post notification targeting

Post access and distribution notifications are independent. Preparing/publishing a post
does not change its existing grants. Notification selection never creates access.

## Publish silently or notify selected grantees

Use `PUT /traces/:trace_id/post` with the existing post fields:

```json
{
  "status": "published",
  "notification_mode": "none"
}
```

Or:

```json
{
  "status": "published",
  "notification_mode": "selected",
  "notification_user_ids": ["reader-uuid", "another-reader-uuid"]
}
```

- `all` (default): existing notification behavior for eligible grantees.
- `none`: no distribution push, instant email, daily fallback or weekly recap.
- `selected`: those channels are restricted to the supplied existing grantees.

Distribution-time mention alerts, and subsequent mention alerts for this post,
also honor this policy, rather than bypassing silent/selected distribution. Explicit
chat messages, history-sharing review/confirmation alerts and other independent
events are not post-distribution notifications and are unchanged.

Recipients' own preferences, blocking, actual current access and platform eligibility
still apply. Selecting someone is not a promise of delivery. Existing deduplication
between mention/post alerts and push/instant email remains in effect.

The two new fields are optional. Omission preserves the post's current settings;
new and migrated posts default to `all`. Switching to `all` or `none` clears any
stored selection. A nonempty recipient list without an effective `selected` mode
is rejected, not silently interpreted as notifying everyone.

`selected` requires 1–500 distinct users, all already active grantees. Duplicates
are removed. Invalid/ungranted/blocked recipients return `400` without publishing.
Prepare the post and its grants first; settings can be configured on that draft
post even while its source trace is still a draft. Revoking access later prevents
delivery even if a UUID remains in the saved selection.

The same options are available on `PUT /documents/:id/post`,
`PUT /albums/:id/post`, and custom-post `PUT /posts/:id`.
`POST /posts` supports `all`/`none`; selected recipients require an existing prepared
post with grants. The post response includes `notification_mode`.

Changing preparation preferences on an already-published post does **not** send
another notification or alter an existing distribution event's fixed recipients.
Repeated `PUT` with `status: published` does not resend. A later distribution
(archived → published) uses the persisted preferences; explicit
`POST /posts/:id/distribution_events` accepts its own notification selection.

## Read the publisher's selection

`GET /posts/:post_id/notification_settings` (publisher only; others receive `404`)

```json
{
  "notification_mode": "selected",
  "notification_user_ids": ["reader-uuid", "another-reader-uuid"]
}
```

This GET returns the latest distribution event's selection when one exists.
The recipient list is intentionally absent from ordinary post/feed responses:
readers should not discover who else was privately notified. Settings belong to
each post, not the trace author globally; another publisher's settings are
independent.

No feed/journal visibility, unread counts, access grants or seen-state behavior changes.
No second post or chat message is created to implement notification selection.
