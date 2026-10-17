# Personal translation messages on traces

Trace read responses now include:

```json
{
  "id": "trace-uuid",
  "content": "Original writing…",
  "translation_message": {
    "id": "mentor-reply-uuid",
    "sender_user_id": "mentor-uuid",
    "recipient_user_id": "current-reader-uuid",
    "trace_id": "trace-uuid",
    "reply_to_message_id": "translation-request-uuid",
    "message_type": "mentor_reply",
    "processing_state": "processed",
    "title": "Traduction française",
    "content": "A small note from the mentor…\n\n## Full translation\n\nTranslated writing…"
  }
}
```

The example abbreviates the existing Message shape. The actual field contains the
complete Message object, including its comments, translation, suggested actions,
metadata, reactions and timestamps. There is no separate translation text record,
source-version field or stale flag. The original trace content is unchanged.

## Selection and privacy

- `translation_message` is always present, either a Message or `null`.
- It is the current requester's latest nonempty, successfully processed mentor reply
  to a `shared_trace_translation_request` for this trace. Ordering is reply
  `created_at DESC, id DESC`.
- Pending/running/failed replies and other mentor message types are excluded. A newer
  incomplete request does not hide an older completed translation.
- The originating request and reply must match the reader, mentor and trace.
- The most recent completed translation is returned regardless of target locale;
  there is no locale filter in this first version.
- Other readers and the trace author do not receive the requester's private message.
  Even a journal owner inspecting another reader's audience-access list only receives
  the owner's own translation, never the inspected reader's conversation.
- Blocked mentor conversations are excluded. Normal trace-read authorization still
  applies; a previous translation does not grant continued access to a revoked trace.
- Hydration does not mark the message or trace as seen.

## Read surfaces

The field is hydrated on single-trace reads, journal trace collections, user trace
collections, `/feed`, `/me/feed`, trace search results, writing-thread trace lists,
album trace items, and complement collections. On feed items it is a top-level field;
document and album feed cards return `null`. On album items it is inside `trace`.
Public journal-sharing views have no personal requester context and return `null`.

A shared batch hydrator handles an entire response page. Message selection/loading
uses one query, and reactions use the existing second batched query; there are no
per-trace message lookups.

## Mentor response order

New translation replies explicitly put the mentor's short comments first, followed
by a clearly separated full translation. Section headings are localized naturally.
Existing stored messages are returned unchanged, not rewritten or regenerated.
The existing model, request payload and response schema are unchanged.

No database migration or new environment variables are required.
