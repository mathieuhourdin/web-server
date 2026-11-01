# Post distribution events and archival trace versions

## Semantics

Traces remain mutable and readers/blogs always display their current content.
`trace_versions` contains immutable, owner-only archival snapshots. A distribution event
can reference a snapshot as historical provenance; that reference does not change
rendering, access grants, analyses or the trace edit-conflict version number.
Snapshots are created automatically when a non-encrypted trace becomes finalized,
and can also be created explicitly. They are not created for every keystroke/edit.

A post remains one placement of a source into social sharing or a blog channel.
A channel post is a durable **publication**; it is not a distribution event.
`post_distribution_events` records `PostDistributionEvent` occurrences. Each event has a server timestamp,
notification selection, optional trace version, and a fixed recipient ledger.
The event and its recipients are recorded transactionally. Adding later grantees
does not silently add them to an old event's recipients.

First distribution and transitions from archived/draft to published automatically
create an event through existing routes. Updating an already-published post or
editing its source does not create another event. Repeated blog publication PUTs
also remain idempotent; use the explicit distribution-event route to bump an existing article.

Posts/feed items include `last_distribution_event_id` and `last_distribution_at`. Feed
ordering uses the last distribution timestamp. `publishing_date` remains the
original/legacy publication date. Unpublished entries retain the existing fallback
date; album cards retain their existing creation-date feed semantics. Blog article
ordering also uses the last distribution timestamp. There remains only one feed
entry per post/source under the established canonical publication rules.

Seen history is not cleared by republishing. Notifications do not create grants.
Archiving, blocking, encryption and permission revocation still govern access.

## Explicit distribution (authenticated post publisher only)

`POST /posts/:post_id/distribution_events`

```json
{
  "idempotency_key": "client-generated-uuid",
  "notification_mode": "selected",
  "notification_user_ids": ["existing-grantee-uuid"],
  "trace_version_id": null
}
```

The post must already be published and its publisher must still be authorized.
`notification_mode`: `all`, `none`, or `selected`. Default: `all` for social posts,
`none` for blog/channel posts. Channels only accept `none`. Selected mode requires
1–500 distinct existing eligible human grantees; other modes require an empty or
omitted recipient list. A supplied snapshot must belong to the post's trace.

The client must reuse its idempotency key when retrying the same action. Repeating
it with identical parameters returns the existing event without another feed bump
or notification dispatch. Reusing it with different parameters returns 409.
Permissions are checked even on idempotent retries.

Response (200 for a new event or an idempotent replay):

```json
{
  "id": "distribution-event-uuid",
  "post_id": "post-uuid",
  "distributed_at": "2026-10-31T12:00:00",
  "trace_version_id": null,
  "notification_mode": "SELECTED",
  "notification_user_ids": ["existing-grantee-uuid"],
  "idempotency_key": "client-generated-uuid",
  "dispatch_started_at": null,
  "created_at": "2026-10-31T12:00:00"
}
```

`dispatch_started_at` is operational progress, not proof of successful delivery.
The immediate POST response may precede dispatch updates. History retrieval
returns current progress. Distribution timestamps/content references and recipient
selection cannot be edited in place.

`GET /posts/:post_id/distribution_events?limit=20&offset=0` returns owner-only history,
newest distribution first, in `{items,total,limit,offset}`.

`GET /posts/:post_id/distribution_events/:distribution_event_id/recipients?limit=20&offset=0`
returns the fixed target list, with `user_id`, `push_status`, `push_sent_at`,
`email_id`, `email_status`, `email_sent_at`. Push statuses are `NOT_ATTEMPTED`,
`SENDING`, `SENT`, `FAILED`, `SKIPPED`; email state comes from the existing mailer.
Targets are distinct from actually notified users. No reader-facing API exposes
the publisher's selection or notification delivery history.

Dispatch uses once-only event and recipient claims. Each distribution event can notify
the same user again; retrying dispatch of that event cannot create a second push
attempt or email queue item. This is not a promise of network-level exactly-once
delivery. There is no automatic recovery/retry of interrupted dispatch in this
version; incomplete attempts remain visible in the ledger.

Push payloads retain the existing event type and add `distribution_event_id`; their
`distributed_at` uses the event time. Push-first/email fallback, preferences and email
rate limits remain. Access is rechecked before push attempts and before processing
distribution emails. An archived/revoked post's email is suppressed.
Mention notifications remain independently deduplicated on the mention.

## Archival snapshots (trace author only)

Finalization automatically inserts a `trace_versions` snapshot in the same database
transaction. It records the final title, subtitle, content, image, language,
`version_integer` and `finalized_at`. This covers ordinary finalization, timeout
finalization, direct finalized creation, imports and complements. Finalized inserts
store their detected language together with content so the snapshot sees it too.

Repeated writes of the same finalized status and later content edits do not create
another automatic version. A later transition back into `FINALIZED` does. Existing
finalized traces are not backfilled. Encrypted traces remain excluded under the
existing archival rules; their ciphertext is not stored as a plaintext version.
Automatic capture does not publish, notify, grant access, freeze the live trace or
automatically attach the version to a distribution event. No extra client request
is required. Snapshots remain immutable and readable only by the trace author.

- `POST /traces/:id/versions`: no body, archive the current finalized, non-encrypted
  trace; returns the snapshot. Does not publish or send notifications.
- `GET /traces/:id/versions?limit=20&offset=0`: paginated snapshots, newest first.
- `GET /traces/:id/versions/:version_id`: one snapshot.

Fields: `id`, `trace_id`, `source_version`, `title`, `subtitle`, `content`,
`content_image_asset_id`, `language_code`, `finalized_at`, `created_at`.
`source_version` is the original trace's optimistic-concurrency version, not a
new editable draft. No update/restore endpoint is introduced. Permanent source
deletion removes snapshots; deleting an asset may clear its archived reference.

## Rename and migration

The naming change is breaking: no old routes, fields, documentation topics or
serde aliases are retained. Events expose `distributed_at`; posts, feed entries
and publications expose `last_distribution_event_id` and `last_distribution_at`.
Push payloads use `distribution_event_id` and `distributed_at` (milliseconds).
`post_published` notification event types and public article URLs remain unchanged.

Management collections are `/users/:id/publications` and
`/blogs/:id/publications`; event history is exclusively
`/posts/:id/distribution_events`. Public `/blog/:slug/posts/:id` article URLs are
unchanged, not aliases for event history.

The rename migration preserves existing IDs, timestamps, snapshots, recipients,
dispatch claims and notification delivery states. It updates queued-email resource
tags to `POST_DISTRIBUTION_EVENT` without requeueing or sending any email/push.
PostgreSQL trigger/function bodies and indexes are updated transactionally. Deploy
the matching backend with the migration: older binaries do not support the renamed
schema. A reverse migration is provided, not a runtime compatibility layer.

Notification fields on posts remain preparation preferences. They do not determine
who receives an already-created event. The owner-only notification-settings GET
returns the latest event's configuration once there is a distribution event.

The original event-ledger migration created one baseline event per previously published post, preserving
its recorded publication date. Known matching successful push deliveries are
retained; unavailable historical recipient lists are not invented. Existing legacy
email records remain intact, without guessing which historical event sent them.
Baseline events are marked already dispatched: migration sends no notifications.
The old push-delivery table remains for rollback but is no longer authoritative.
