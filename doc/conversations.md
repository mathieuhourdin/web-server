# Conversations and message-to-trace sources

Authenticated API. Conversations live in `social/conversation`; existing direct
and mentor messages use the same `messages` table as group messages. No per-user
message copies, WebSockets, group AI, delivery-receipt table or stored unread counts.

## Conversations

```
POST  /conversations
GET   /conversations?limit=20&offset=0
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

Collection envelope: `{items,total,limit,offset}` (default 20, max 100). Ordered
by newest readable processed message's creation date, then creation date and ID;
membership/title changes do not bump a group above active conversations.

Each item/detail returns:

```json
{
  "id":"conversation-uuid",
  "kind":"group",
  "title":"Our conversation",
  "created_by_user_id":"creator-uuid",
  "created_at":"2026-11-05T10:00:00",
  "updated_at":"2026-11-05T10:02:00",
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

Group AI requests and structured attachments are rejected in this first version.
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
