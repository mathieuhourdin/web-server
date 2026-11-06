# Mentor feedback style

An account-level preference, independent of the selected mentor's identity:

- `balanced` (default): precise, proportionate observations; strengths and weaknesses
  when warranted, with no praise/criticism quota.
- `challenging`: more readily test assumptions, identify weak decisions, arguments or
  literary choices, and suggest concrete alternatives. Not harsh or contrarian by default.
- `companionable`: more conversational and exploratory, less directive; still candid,
  specific, and willing to name important weaknesses.

Every style is honest. Literary or humorous material can receive direct craft criticism
without turning into therapy or a judgement of its writer. Praise is specific and earned;
neither criticism nor personal-development advice is mandatory.

## API

Use the existing authenticated account routes; no new endpoint:

```http
PATCH /users/:my_user_id
Content-Type: application/json

{"mentor_feedback_style":"challenging"}
```

Only the account owner can update this field. The response is the usual updated
user object, including `mentor_feedback_style`. An omitted or null value leaves the
preference unchanged; explicitly send `balanced` to reset it. Unknown enum values
are rejected. Existing `PUT /users/:my_user_id` also accepts this optional field
alongside its existing profile payload; omission preserves the saved preference.

`GET /users/:my_user_id` returns the field in the authenticated self response:

```json
{"id":"my-user-uuid","mentor_feedback_style":"challenging"}
```

Other user fields above are omitted for brevity. Public profile/search/member
projections do not expose the setting. Full self user objects returned at signup,
login and update also contain it. Existing clients may ignore the additive field.

## Generation

The recipient's preference applies to future daily/weekly mentor feedbacks and
generated mentor replies: ordinary messages, finalized traces, draft/thread guidance,
full-journal feedback, tarot, and shared-trace explanation/translation commentary.
The reader's preference is used for shared traces, not the source writer's.

The selected style is included in the prompt context and applied through one shared
system-prompt policy. The mentor identity still supplies voice and perspective.
An explicit current request (for example “only correct the spelling” or “be gentle
this time”) takes precedence over the default style without changing the saved setting.
Instructions embedded in source writings are not treated as such a request.

The shared system-policy and typed user-context split follows the
[official OpenAI prompting guidance](https://developers.openai.com/api/docs/guides/prompting).

Style never changes translation fidelity, the output JSON contract, AI-consent checks,
quotas, model choice or reasoning settings. It does not regenerate previous feedbacks,
start an LLM call on profile update, or change static welcome messages. No free-text
user instruction field or per-conversation preference is introduced in this version.

## Persistence and rollout

Migration `2026-11-06-100000_mentor_feedback_style` adds a non-null text column to
`users` with an explicit constraint. API values are lowercase; database values are
`BALANCED`, `CHALLENGING`, `COMPANIONABLE`. Existing and new users default to
`balanced`. Changing mentor does not reset the preference.

Apply the normal migration before serving the new binary. No new environment
variables, provider calls or scheduled workers. Web/mobile settings UI is separate
and can simply PATCH the existing user resource and refresh its self-user state.
