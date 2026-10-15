# Mentor questions about a writing draft

Use the existing mentor-message API. Save the draft first, then send its saved
version. No new message type or conversation resource is introduced.

## Create

`POST /traces/:draft_id/messages`

```json
{
  "recipient_user_id": "mentor-uuid",
  "message_type": "question",
  "content": "Où en suis-je dans cette réflexion, et que pourrais-je explorer ensuite ?",
  "expected_version_integer": 4,
  "writing_thread_id": "thread-uuid"
}
```

`writing_thread_id` is optional: by default the server uses the thread whose pending
trace is this draft. A standalone draft works without a thread. An explicit thread
must belong to the caller and either hold this pending draft or end at its immediate
predecessor; unrelated/sibling branches cannot be added as context.

The same options are supported by `POST /messages`, with `trace_id` in the body.
Omitted `message_type` defaults to `question` when the recipient is a service mentor.
`expected_version` is accepted as an alias for `expected_version_integer`.
`reply_to_message_id` still works for conversational replies.

For **draft** mentor questions, `expected_version_integer` is now required. Existing
clients must save first and provide the returned version. These two draft-context
options are not supported for human messages, tarot or finalized-trace questions.
Existing finalized-trace questions without these options are unchanged.

The caller must own an ordinary, unencrypted draft in an active, unencrypted journal.
The recipient must be a service mentor. Existing AI opt-out, admin disablement,
quota and delegated-agent restrictions apply. This operation does not finalize,
publish, edit or grant access to the draft.

Response: the existing `200` creation envelope:

```json
{
  "question_message": {
    "id": "question-uuid",
    "message_type": "question",
    "trace_id": "draft-uuid",
    "metadata": {
      "mentor_feedback": null,
      "draft_trace": {
        "trace_id": "draft-uuid",
        "trace_version_integer": 4,
        "writing_thread_id": "thread-uuid",
        "thread_input_revision": 2,
        "omitted_thread_trace_count": 0
      }
    }
  },
  "pending_reply_message": {
    "id": "reply-uuid",
    "message_type": "mentor_reply",
    "processing_state": "pending"
  }
}
```

Examples show only relevant fields; the actual response returns complete Message
objects. Both messages carry the same `metadata.draft_trace` provenance. Standalone
drafts have null thread ID/revision. Fetch the reply using the existing message
routes; existing mentor-message push notifications and LLM token/cost tracking apply.

Errors:

- `400`: missing saved version, empty question, or unsupported context options.
- `409`: version mismatch (details include expected/current version), or a trace
  that is no longer a draft when draft options are provided. Save/reload and ask again.
- `403`: unavailable/encrypted draft or AI disabled.
- `404`: requested draft/thread not available to its owner or not related to this draft.
- `413`: draft/question/thread-analysis metadata alone exceeds the context budget.
- `429`: existing mentor-question quota exhausted.

## Generation and persistence

A repeatable-read transaction captures saved draft text, version, question, thread
analysis and finalized, unarchived, unencrypted thread writings in chain order.
It also includes up to ten recent processed messages with this mentor about this
draft. Source writings override potentially stale AI thread analysis.

Context is bounded to roughly 100,000 serialized Unicode characters, with at most
200 prior traces. When necessary, the newest complete traces are retained; individual
trace text is never silently clipped. Omitted traces are counted and the mentor is
instructed to disclose incomplete historical context rather than invent it.

`mentor_message_contexts` stores this private JSON snapshot, atomically with the
question message. It is not returned by trace/message endpoints. It cascades on
message deletion. Later edits or thread merges do not alter the input snapshot.
No new background worker is introduced: the existing asynchronous message-processing
flow generates the reply. It checks AI consent again before generation and before
persisting the completed draft reply.

The dedicated prompt identifies where the writing is now and, when asked, suggests
two or three grounded possible directions. It respects literary/humorous modes,
avoids unsolicited self-help framing, and does not finish the draft unless asked.

Mobile/web flow: save → await saved version → ask mentor → show pending message →
refresh using the existing message flow. A `409` should offer reload/save, not an
automatic retry against another version.
