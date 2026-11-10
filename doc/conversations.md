# Conversations and message-to-trace sources

Authenticated API. Conversations live in `social/conversation`; existing direct
and mentor messages use the same `messages` table as group messages. No per-user
message copies, WebSockets, group AI, delivery-receipt table or stored unread counts.

## Conversations

```
POST  /conversations
GET   /conversations?limit=20&offset=0
GET   /conversations?q=weekend&limit=20&offset=0
GET   /conversations/:id
PATCH /conversations/:id
```

Create a direct conversation (200, reuses the unordered pair's existing ID):

```json
{"kind":"direct","member_user_ids":["partner-uuid"]}
```

Create a group (201, always a new group):

```json
{"kind":"group","title":"Our conversation","member_user_ids":["user-uuid-1","user-uuid-2"]}
```

Creator is included automatically as an administrator. Provide 1–49 distinct
other users; maximum 50 active members. Group titles are nullable, at most 200
characters. Direct membership is fixed and titles cannot be assigned to direct
conversations. Adding someone requires an accepted follow relationship in either
direction with the inviter; participants do not all need to follow each other.
Mentors remain available as direct partners but mentors/service users cannot
join groups. Demo accounts cannot mix agent sandboxes or join real-user groups.

Collection envelope: `{items,total,limit,offset}` (default 20, max 100). Ordered by
`sort_at DESC`, then conversation creation date descending and ID ascending.
`sort_at` is the newest readable processed-message creation date, falling back to
the **caller's own `membership.joined_at`** if there are no such messages. An old
group newly joined therefore does not sink to its original creation date when
none of its messages are readable yet. Rejoining resets that fallback to the new
admission. Title, mute and other members' membership changes do not bump a chat.

Optional `q` searches conversation titles and **other active members'** current
public display names and handles, case- and accent-insensitively. It does not
search message content, former members, hidden real names of pseudonymized users,
or the caller's own name. Matching is a literal substring (not typo matching;
`%` and `_` are not wildcards). Leading/trailing whitespace is trimmed; empty or
omitted `q` is the unfiltered listing. Maximum 200 characters, otherwise 400.
Access checks, date ordering, response shape and pagination remain identical;
`total` is the number of matching accessible conversations before pagination.

Each item/detail returns:

```json
{
  "id":"conversation-uuid",
  "kind":"group",
  "title":"Our conversation",
  "created_by_user_id":"creator-uuid",
  "created_at":"2026-11-05T10:00:00",
  "updated_at":"2026-11-05T10:02:00",
  "sort_at":"2026-11-05T10:00:00",
  "members_count":3,
  "members_preview":[{"user":{"id":"user-uuid","display_name":"Example","profile_picture_display_url":null},"role":"admin","joined_at":"2026-11-05T10:00:00"}],
  "membership":{"role":"member","joined_at":"2026-11-05T10:00:00","history_start_position":1,"last_read_position":4,"muted_at":null},
  "last_message":null,
  "unread_count":0
}
```

`last_message` is the ordinary Message object when one is readable; otherwise
null. `members_preview` has up to four active participants, each using the usual
public-user response (example above abbreviated). Other participants' mute/read
cursors are not exposed. No per-conversation user/message/profile N+1 hydration.

### Date fields

- `created_at`: conversation creation time. For migrated direct chats, the first
  legacy message's creation time was used.
- `updated_at`: conversation-row update time, changed by new messages or a title
  edit. It is not a per-viewer activity/sort date and does not track every member
  setting or membership change.
- `sort_at`: derived, caller-specific date described above, returned on both list
  and detail responses. This adds no persisted column or migration.
- `membership.joined_at`: caller's current admission time; reset on rejoining.
  Migrated direct memberships use the conversation's initial creation time.
- `membership.muted_at`: nullable timestamp when the caller muted the chat.

These timestamps follow the existing UTC-without-offset response convention.
Messages additionally retain their own `created_at`, `updated_at` and direct
`seen_at`; membership `left_at` is an internal access-revocation timestamp.

PATCH accepts `{"title":"New title"}` (group admins only), `{"title":null}`
to clear, and/or `{"muted":true|false}` for the caller's own notification setting.

## Membership

```
GET    /conversations/:id/members?limit=20&offset=0
POST   /conversations/:id/members                 {"user_ids":["uuid"]}
PATCH  /conversations/:id/members/:user_id        {"role":"admin"}
DELETE /conversations/:id/members/:user_id
```

GET lists active members, ordered by joining date and user ID, with the same
pagination envelope. POST/PATCH/DELETE return 204. Admins add/remove members and
manage roles; any member can remove themselves to leave. A last admin cannot
leave/be demoted while other members remain unless another admin is appointed.
Account deletion automatically promotes the oldest remaining active member when
necessary. Empty groups have no active members and cannot be reopened by outsiders.

### Shared conversation nicknames

Use the existing `PATCH /conversations/:id/members/:user_id`, which returns 204:

```json
{"nickname":"Captain"}
```

Any current member can nickname themselves or another active member, in direct
and group conversations. These labels are shared with all members, not private
address-book aliases. The global User profile, handle and authorship are unchanged.
Treat nicknames as plain text; clients may prefer `member.nickname` for contextual
display while keeping profile/handle identity available.

`{"nickname":null}` or a blank string clears it; omission preserves it. Values
are trimmed, limited to 80 Unicode characters, and cannot contain control
characters. An empty PATCH is 400. Changing a non-active target or acting without
conversation access is forbidden. `role` remains optional and admin-only for
groups; combining role and nickname is atomic (a refused role change must not
partially save the nickname). Existing role-only requests remain valid.

`nickname: string|null` is returned beside `user` in each member-list and
`members_preview` item, and on the caller's own `membership` object, including
read-cursor responses. It never replaces `user.display_name`. Old messages do not
store a nickname snapshot: clients resolve the participant's current label.
Changing a nickname doesn't bump conversation sort/activity dates, create a
message, increment unread counts, or send a push/email. It emits SSE
`conversation_updated`, prompting member metadata refresh. Existing push display
names remain the global profile names. Rejoining clears the old nickname.

Direct additions require no invitation acceptance. New members see messages
posted **since joining**, not previous history. Joining, rejoining and sending
serialize on the conversation row. Rejoining starts a fresh history boundary and
does not restore earlier messages, including messages the person authored before
leaving. Adding an already-active member does not reset their history or role.

Leaving/removal revokes all conversation/message access immediately, including
detail reads, replies, reactions, reporting and choosing new trace sources. IDs
alone and message authorship do not bypass membership. Admins do not bypass a
joining-history boundary either. Group titles/current participants remain group
metadata rather than a full membership-event history.

Blocking prevents direct conversations and new admissions involving a blocked
pair (either direction); failed group additions use a generic error. Blocking
after people already share a group does not eject either person or hide their
group messages. Pushes triggered by the blocked actor's messages/reactions are
suppressed in both directions. A member can still leave or be removed by an admin.

## Sending and reading

```
POST /conversations/:id/messages
GET  /conversations/:id/messages?limit=20&before_position=123
PUT  /conversations/:id/read
```

```json
{"content":"Hello","reply_to_message_id":"optional-message-uuid"}
```

POST returns 201:

```json
{"message":{"id":"message-uuid","conversation_id":"conversation-uuid","position":12,"sender_user_id":"user-uuid","recipient_user_id":null},"pending_reply_message":null}
```

Message object above is abbreviated; all existing content, attachments, metadata,
processing state, timestamps and reactions remain present. `recipient_user_id`
is UUID for direct messages and null for groups. `conversation_id`/`position` are
also added to messages returned through legacy endpoints. Positions are unique,
increasing within a conversation and allocated in the insertion transaction;
failed inserts do not commit a new head. Deletions can leave position gaps.
Message participants/conversation/position are immutable.

Groups accept ordinary text messages (nonblank, at most 50000 characters), optional
title, optional `reply_to_message_id`, and either `trace_id` or `post_id`. Every
active member must currently be able to read a linked source. Joining a group
does not grant access to its linked traces/posts; subsequent source reads retain
their own ACLs. Group replies must target a processed message in the same group
that the sender can currently read. Group replies do not automatically inherit
source context or expose quoted text from messages outside another member's history.

Group AI requests and mentor structured attachments are rejected. Temporary image
attachments are supported as described below.
For direct conversations, sending delegates to the existing message engine and
accepts its optional `message_type`, `attachment_type`, `attachment`,
`writing_thread_id`, `expected_version_integer`, and `landscape_analysis_id`,
preserving mentor behavior/AI consent and usage guards. The older dedicated
trace/journal mentor endpoints also continue to work.

GET returns processed messages newest-position-first in the usual collection
envelope. `total` counts all messages currently visible in the conversation, not
just those before the cursor. Next page: use the last returned position as
`before_position`. Offset must be zero; an invalid cursor returns 400.

Mark read explicitly, never by fetching:

```json
{"through_position":123}
```

Returns the caller's membership projection. Cursor advances monotonically and
must be between `history_start_position - 1` and the current conversation head.
The cursor is conversation-global, not limited to the currently loaded page;
marking through a position acknowledges all preceding visible incoming messages.
Unread counts exclude the sender's own messages and non-processed messages.
Pending/running mentor placeholders hold back direct read cursors, so completing
a reply later does not silently mark unread content as read. Legacy direct
`seen_at` values continue to account for sparse per-message read history.

Existing `GET /messages/:id`, reaction and report routes enforce the same
membership/history checks. `PUT /messages/:id` cannot change participants and
remains a direct-message operation; group message editing is not included yet.

## Notifications and old clients

Newly added group members also receive a dedicated `conversation_member_added`
push, including initial participants when a group is created. The creator is
excluded. Rejoining produces a new event; adding an already-active member does
not. The transaction returns the actual admissions, and the route schedules
pushes only after it commits. No synthetic chat message or unread item is created.

FCM data (all values are strings):

```json
{"event_type":"conversation_member_added","conversation_id":"uuid","conversation_kind":"group","conversation_title":"Weekend plans","actor_user_id":"uuid","actor_display_name":"Mathieu","actor_avatar_url":"https://...","event_timestamp":"1793952000000"}
```

`conversation_title` and `actor_avatar_url` are omitted when unavailable.
`event_timestamp` is the admission time in UTC epoch milliseconds. Display uses
the actor's name with “vous a ajouté·e à « Weekend plans »”, or “vous a ajouté·e
à une conversation” for an unnamed group. On tap, open `conversation_id`.
APNs `thread-id` is the conversation UUID. Current membership, the exact admission,
mute and actor/recipient blocks are rechecked before dispatch; old queued events
are discarded if someone leaves and rejoins. Delivery is best-effort, push-only,
with no new outbox, retry worker or email fallback.

New group messages fan out one `message_received` push per eligible active member,
excluding sender, muted members and blocked actor/recipient pairs. Eligibility is
rechecked before dispatch; already-delivered pushes cannot be revoked. Group
notifications contain a short content preview and have no email fallback yet.

Existing message payload keys remain. Added FCM data:

```json
{"conversation_id":"uuid","conversation_kind":"group","message_position":"12","conversation_title":"Our conversation"}
```

`conversation_title` is included for named groups. Direct message pushes also carry
conversation ID/kind/position. The existing FCM-safe `hupo_message_type` key is
retained (FCM reserves `message_type`). iOS `thread-id` uses the conversation UUID,
including reaction notifications, which also include `conversation_id`. Group
reactions notify the original author only, respecting their membership/mute/block
settings; one reaction per user/message remains unchanged.

Legacy `/messages`, `/messages/conversations`, and partner/context listings remain
direct-only adapters. Existing direct/mentor writes automatically acquire their
unique conversation and position at the database boundary, including cron/system
feedbacks. Global `/me/unread_counts` includes group unread messages within its
existing time window. Clients need new screens/state/routing for groups; this
backend change does not automatically add group support to older clients.

## Creating a trace from messages

Existing trace creation and version-checked PATCH/PUT routes accept:

```json
{"content":"The author's chosen or rewritten text","source_message_ids":["message-uuid-1","message-uuid-2"],"expected_version_integer":4}
```

No server-generated text or special trace type. This is a `trace_message_sources`
relation, not a live copy or ACL transfer. On updates: absent preserves links,
empty array clears them, nonempty array replaces them. At most 100 distinct source
IDs; newly linked messages must be processed and currently readable by the trace
owner. Sources may span conversations; no extra conversation FK is needed.
Text and links save atomically under the existing trace version check. Failure
does not partially save content. No equality check against original message text.

Existing links survive subsequent access loss, and the saved trace remains editable.
The trace owner sees source IDs in owner trace/detail/list/feed responses; other
readers do not receive them. Fetching original messages still requires current
conversation access. No message content, group name or participant list is embedded
in provenance. Source ordering follows conversation ID and message position.

Sharing the new trace is a deliberate user copy/export action, including when it
contains another participant's words. The client should make that consequence
clear when offering the action. No automatic grants or publication are created.
Deleting a source message removes its link, not the trace or its supplied text;
deleting the trace removes its links.

## Deployment and verification

Migration `2026-11-05-100000_conversations` backfills one direct conversation per
unordered legacy participant pair, ordered positions and conservative read
cursors without silently acknowledging unread content. An additive trigger also
covers all future legacy/system inserts. Downgrade refuses to discard groups or
trace provenance. No new environment variables, cron jobs or external services.
Once group messages exist, do not roll back to an old server binary that assumes
every message has a non-null recipient; use a conversation-aware rollback release.
Roll out updated web/mobile clients separately. No LLMs or real notification sends
are required to verify the synthetic database scenario.

## Temporary conversation images

Images have their own `message_images` table; they are NOT permanent platform
Assets and cannot be used as trace/document/profile images. No client changes are
included in this backend feature. The existing mentor `attachment_type` and
`attachment` fields remain unchanged.

### Upload, then send

```
POST /conversations/:id/images
Content-Type: multipart/form-data
file: <one image>
```

201, example (timestamps are UTC):

```json
{
  "id":"image-uuid",
  "mime_type":"image/jpeg",
  "size_bytes":182400,
  "width":1600,
  "height":1200,
  "created_at":"2026-11-10T10:00:00",
  "expires_at":"2026-12-25T10:00:00",
  "remote_available":true
}
```

This is an authenticated, server-mediated upload, not a signed upload URL.
Exactly one multipart field named `file`; bytes determine the actual MIME type,
not the supplied filename or MIME header. JPEG, PNG and WebP only, 5 MiB maximum,
8192 pixels per side and 16 million pixels total. The server checks dimensions
and decodes under allocation limits. Clients must resize/compress before upload,
apply orientation and remove EXIF/location metadata if desired; the backend does
not transform the bytes or strip metadata. Animation is not a supported feature.

Uploads require current human-conversation access. Direct chats also require the
usual accepted relationship; service/mentor direct chats do not support images.
There are at most 100 reservations per rolling 24 hours per user and two upload
handlers per API process. Quotas include failed/cancelled reservations in the
daily budget. 429 means retry later. Lost pending-upload IDs do not cause a
45-day lockout: only the rolling daily upload budget is enforced.
503 means the temporary bucket is not configured. Invalid images return 400;
oversized payloads can return 413. Uploading creates neither a message nor a push.

Send using the existing route:

```http
POST /conversations/:id/messages
Content-Type: application/json

{"content":"Here are the photos","image_attachment_ids":["image-uuid"]}
```

Up to four distinct IDs, ordered as provided. `content:""` is valid for image-only
messages. IDs must be READY, unexpired, unsent, uploaded by this sender for this
conversation during their current admission. Leaving and rejoining invalidates
previous pending uploads. Sending claims all images in the same DB transaction
as message insertion/position allocation; failure rolls everything back (409 for
an unavailable/wrong-owner/wrong-conversation/already-sent image, 400 for duplicate
or too many IDs). Images cannot be attached to mentor requests. Legacy
`POST /messages` also accepts `image_attachment_ids` for ordinary human messages;
it resolves the same direct conversation. Other message creation routes have not
gained an image field. Image associations are immutable after sending: current
message edits cannot add/remove/replace images.

### Message responses and downloads

All normal message collection/detail/create responses, including conversation
`last_message` and legacy message listings, include `images: []` or the ordered
metadata objects shown above. Hydration is batched per message page. Bucket/object
keys, upload status and signed URLs are not embedded. Expired metadata remains in
PostgreSQL, with `remote_available:false`; fetching messages does not contact GCS.
`remote_available` represents the backend's retention state, not a live GCS HEAD
check. Unexpected object deletion may still cause a download 404.

```
GET /conversations/:id/images/:image_id/url
```

200:

```json
{"url":"https://storage.googleapis.com/...?signature=...","expires_at":"2026-11-10T10:05:00"}
```

URL `expires_at` is its short validity window, not the image retention deadline.
URLs last at most five minutes and never extend beyond remote retention. Every
request checks access to the specific sent message using existing membership,
blocking and joining-history semantics. Even uploaders cannot bypass these rules;
unsent images cannot be downloaded through this route. A former/newly joined
member cannot download old images by guessing IDs. Missing/unsent images: 404;
inaccessible messages: 403; authorized but expired images: 410. Obtain a fresh URL
after URL expiry; show an unavailable placeholder on remote expiry/object 404.
Already issued URLs are bearer credentials until expiry and cannot be revoked
immediately by leaving/blocking. Do not log or persist URLs as image identity.

```
DELETE /conversations/:id/images/:image_id
```

204 cancels an owned unsent upload. It first disables sending, then deletes the
object; repeated cancellation is safe. Current conversation access is required.
Sent images cannot be cancelled (409); cancellation is not message editing.
Metadata remains for quota accounting/account purge. Orphan objects from abandoned
uploads or failed cleanup still expire through GCS.

### Storage, notifications and client ownership

Remote lifetime is 45 days from the upload reservation, not from message sending
or download. GCS lifecycle removes bytes asynchronously; the backend independently
enforces its precise `expires_at`. It does not run an expiry/cleanup cron or
permanently remove metadata on expiry. GCS versioning/soft delete must be disabled
on the dedicated temporary bucket; no retention lock. Storage objects are private,
create-only and use `Cache-Control: private, no-store`. Mobile's deliberate private
filesystem persistence is separate from HTTP caching.

Sending uses existing message push/email behavior. Image-only previews say
`📷 Photo` or `📷 N photos`; FCM `message_image_count` is a string. There are no image
URLs/bytes in notification payloads. Existing mute/block/member checks remain.

Account purge deletes owned temporary objects before deleting DB metadata, treating
already-removed objects as success. Explicit message deletion can orphan its
metadata (the message FK is set null), but cannot make its image reusable. Pending
upload cleanup is not a new scheduled job. Database backups preserve metadata,
not expired image bytes.

Mobile should identify cached files by image UUID, scope them to the signed-in
account, prefer its local file before checking remote availability, and only
download readable messages when the conversation is open. Web uses signed URLs
without deliberate durable storage. Neither server access revocation nor GCS
expiry can erase already-downloaded copies. This is not end-to-end encryption.

Deployment: apply migration `2026-11-10-100000_message_images`, create dedicated
dev/prod buckets using `ops/conversation-images/`, and configure
`GCS_CONVERSATION_IMAGES_BUCKET`. Existing ADC/signing infrastructure is reused;
no service-account keys or new cron jobs are needed. Never point this variable at
the permanent/private or public asset buckets. Do not apply TTL rules to those
existing buckets. Uploads fail closed until configured; existing text chats work.

## Live conversation events (SSE)

```
GET /me/conversation_events
Authorization: Bearer <normal-session-token>
Accept: text/event-stream
```

One foreground connection covers all the caller's accessible conversations.
Writes still use the existing HTTP routes. Push delivery and mentor processing
are unchanged; SSE is an additional best-effort acceleration channel, not a
durable queue. No clients are changed by this backend implementation.

The response is `text/event-stream`, `Cache-Control: private, no-store`, with
`X-Accel-Buffering: no`. The first frame is always:

```text
event: resync_required
data: {"reason":"connected"}

```

No event IDs, replay cursor or `Last-Event-ID` guarantee. Establish the stream
before fetching current state and buffer/merge arriving events while the fetch
completes. Otherwise a fetch/subscribe race can silently miss a message. Upsert
messages by ID; duplicate or coalesced events are expected. A delayed event loads
the latest message state, not a historical snapshot.

### Events

`message_created`, `message_updated`, and `reaction_updated` carry:

```json
{
  "conversation_id":"conversation-uuid",
  "message":{
    "id":"message-uuid",
    "conversation_id":"conversation-uuid",
    "position":12,
    "content":"Hello",
    "reactions":[],
    "images":[]
  }
}
```

`message` is the full existing Message response (example abbreviated), including
viewer-specific reactions and temporary image metadata, but no image URLs.
Membership/history and session validity are checked again at projection time.
Muted conversations still produce foreground events. Blocking follows existing
read semantics: direct access is revoked; existing shared-group messages remain
readable even between blocked participants, unlike their suppressed pushes.

Pending mentor inserts aren't emitted as new messages. When a worker finishes
one, it emits `message_created` at its original position; insert/upsert by position
rather than blindly appending. Other processing-state changes can emit
`message_updated`. This does not stream LLM tokens. Read/seen acknowledgements do
not emit message-edit events.

`message_deleted` carries only:

```json
{"conversation_id":"uuid","message_id":"uuid","position":12}
```

This reflects actual database deletion and does NOT introduce a delete-for-
everyone API. Current conversation access and the original position's admission
boundary are checked before exposing these identifiers. Existing saved trace
copies are not erased.

`conversation_updated` carries `{"conversation_id":"uuid"}`. Refresh conversation
and member metadata through the normal API. It covers title, shared nickname,
membership/role and own read/mute-state changes. Other members' private read/mute
cursors are never embedded or broadcast. `conversation_removed` carries the same
shape, only to the affected former member: evict/invalidate that conversation.

`resync_required` carries `{"reason":"..."}`. Possible reasons include `connected`,
`listener_connected`, `listener_disconnected`, `client_lagged`,
`listener_overflow`, `projection_failed`, `stream_closed`, `access_changed`, and
`invalid_notification`. Reasons are diagnostic, not a closed client enum: handle
unknown reasons by resynchronizing too. Refresh loaded message windows as well
as conversation/unread state; fetching only newer positions misses changes to
older messages. Use existing paginated reads for this MVP. No `after_position`
or `around_message_id` API has been added as part of SSE.
Coalesce resync requests while a refresh is in flight; an import/burst can produce
several gap signals and must not cause one full reload per signal.

`authentication_expired` carries `{}` and ends the stream. Stop reconnection with
the same invalid token and follow normal authentication handling. Sessions are
rechecked before data projection and every 30 seconds while idle. An already
authorized/in-flight frame cannot be recalled from a client.

### Reconnection and operation

- SSE keepalive comments arrive every 15 seconds; they are not message events.
- On EOF/network failure, reconnect with backoff and jitter (e.g. 1–30 seconds),
  then perform the same full-window resync. On app resume always reconnect/resync;
  background push is still needed while mobile suspends the connection.
- Browser-native EventSource cannot supply arbitrary bearer headers. Use a
  streaming fetch/SSE parser with Authorization. Never put tokens in query strings.
- 401 means invalid session; 429 means the process's connection limit was reached;
  503 means the listener isn't ready/reconnecting. Respect backoff, not rapid loops.
- Limits: five streams per user, 256 total per API process; 64 queued identifier
  events per user. Dropping a stream releases its slot. Slow clients receive a
  resync signal after lag instead of unbounded buffering.

Implementation is feature-local in `social/conversation/events`. The listener
starts once after migrations and uses one dedicated `tokio-postgres` connection,
not a held request-pool connection for each client. It reconnects automatically;
all subscribers are told to resync after listener gaps. Notification intake is
bounded too. Trigger payloads contain IDs only, never message text, nicknames,
titles or credentials. Connection/payload details are not logged. For this VM
deployment it uses the existing trusted-network PostgreSQL connection without
TLS; a future TLS-required DB needs an appropriate listener TLS connector (it
fails closed rather than silently disabling a required TLS mode). A transaction-
pooling proxy cannot supply this dedicated LISTEN session; use a direct/session-
pooled DB endpoint if infrastructure changes.

There is no event-history table, outbox, per-device delivery cursor, Redis or
new cron. PostgreSQL delivers notifications at commit, including writes from
mentor workers and legacy routes. Events can be lost/coalesced across failures;
the normal API/database remain authoritative. Logs use target
`conversation_events` with connected/disconnected/projection-failure events.
Request-header timing excludes the lifetime of the SSE body; nginx request timing
includes that lifetime and should not be interpreted as a slow ordinary query.

Deployment: migrations `2026-11-10-110000_conversation_events` (functions/triggers)
and `2026-11-10-120000_conversation_nicknames` (one membership field + trigger).
No new environment variable or cloud resource. The local nginx config includes
the endpoint; the production server block also needs
`ops/nginx/conversation-events.location.conf` adapted to its existing upstream,
then `nginx -t` and reload. Do not replace production TLS/host configuration with
the local example. No production config is changed automatically by these files.
