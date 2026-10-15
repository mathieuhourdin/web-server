# Publication notification targeting

Publication access and notifications are independent. Preparing/publishing a post
does not change its existing grants. Notification selection never creates access.

## Publish silently or notify selected grantees

Use `PUT /traces/:trace_id/post` with the existing publication fields:

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
- `none`: no publication push, instant email, daily fallback or weekly recap.
- `selected`: those channels are restricted to the supplied existing grantees.

Publication-time mention alerts, and subsequent mention alerts for this publication,
also honor this policy, rather than bypassing silent/selected publication. Explicit
chat messages, history-sharing review/confirmation alerts and other independent
events are not publication notifications and are unchanged.

Recipients' own preferences, blocking, actual current access and platform eligibility
still apply. Selecting someone is not a promise of delivery. Existing deduplication
between mention/publication alerts and push/instant email remains in effect.

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

Changing settings on an already-published post does **not** send another notification.
It affects future notification/recap processing, not notifications or emails already
sent/enqueued. Repeated `PUT` with `status: published` does not resend. A later real
republication (archived → published) uses the persisted settings unless overridden.

## Read the publisher's selection

`GET /posts/:post_id/notification_settings` (publisher only; others receive `404`)

```json
{
  "notification_mode": "selected",
  "notification_user_ids": ["reader-uuid", "another-reader-uuid"]
}
```

The recipient list is intentionally absent from ordinary post/feed responses:
readers should not discover who else was privately notified. Settings belong to
each publication, not the trace author globally; another publisher's settings are
independent.

No feed/journal visibility, unread counts, access grants or seen-state behavior changes.
No second post or chat message is created to implement notification selection.
