# Quick API Documentation

Source of truth:
- `src/router.rs`
- current DTOs and route handlers under `src/entities_v2/**`

This file is a practical quick reference for the current server contract.

**Conventions**
- Base URL: server root, examples assume `/`
- Auth token: `Authorization: Bearer <session_id>.<secret>`
- UUID path params are written as `:id`, `:user_id`, etc.
- Repeated query params on the same field use OR semantics
  - example: `?interaction_type=inpt&interaction_type=outp`
- Different query fields combine with AND semantics

**Pagination**
- Standard query params:
  - `limit`
  - `offset`
- Defaults:
  - `limit = 20`
  - `offset = 0`
- Max `limit = 100`
- Standard paginated response:

```json
{
  "items": [],
  "limit": 20,
  "offset": 0,
  "total": 0
}
```

## Main Payloads

### Session Login
```json
{
  "username": "string",
  "password": "string"
}
```

### New User
```json
{
  "email": "string",
  "principal_type": "human|service|null",
  "mentor_id": "uuid|null",
  "first_name": "string",
  "last_name": "string",
  "handle": "string",
  "password": "string|null",
  "profile_picture_url": "string|null",
  "is_platform_user": "bool|null",
  "biography": "string|null",
  "pseudonym": "string|null",
  "pseudonymized": "bool|null",
  "high_level_projects_definition": "string|null",
  "journal_theme": "classic|white|flowers|dark|null",
  "current_lens_id": "uuid|null",
  "week_analysis_weekday": "monday|...|sunday|null",
  "timezone": "string|null",
  "context_anchor_at": "datetime|null",
  "welcome_message": "string|null",
  "home_focus_view": "projects|follows|drafts|null"
}
```

### New Trace
```json
{
  "journal_id": "uuid",
  "content": "string",
  "interaction_date": "datetime|null",
  "is_encrypted": "bool|null",
  "encryption_metadata": "json|null"
}
```

### Update Trace
```json
{
  "content": "string|null",
  "interaction_date": "datetime|null",
  "status": "draft|finalized|archived|null"
}
```

### Patch Trace
```json
{
  "content": "string|null",
  "interaction_date": "datetime|null"
}
```

### New Post
```json
{
  "source_trace_id": "uuid|null",
  "title": "string",
  "subtitle": "string|null",
  "content": "string",
  "image_url": "string|null",
  "post_type": "idea|ratc|book|curs|quest|oatc|pblm|pcst|natc|rdnt|list|null",
  "interaction_type": "outp|inpt|pblm|wish|null",
  "publishing_date": "datetime|null",
  "status": "draft|published|archived|null",
  "audience_role": "default|restricted|null"
}
```

### New Trace Post
`POST /traces/:id/posts` accepts the same post fields except `source_trace_id` is forced from the path, and these fields are optional:
- `title`
- `subtitle`
- `content`

When omitted, they default from the source trace.

### New Journal
```json
{
  "title": "string",
  "subtitle": "string|null",
  "content": "string|null",
  "is_encrypted": "bool|null",
  "journal_type": "meta_journal|work_log_journal|reading_note_journal|null"
}
```

### Update Journal
```json
{
  "title": "string|null",
  "subtitle": "string|null",
  "content": "string|null",
  "is_encrypted": "bool|null",
  "journal_type": "meta_journal|work_log_journal|reading_note_journal|null",
  "status": "draft|published|archived|null"
}
```

### Journal Export
```json
{
  "format": "md|txt|json",
  "include_messages": true
}
```

### New Lens
```json
{
  "processing_state": "out_of_sync|in_sync|null",
  "fork_landscape_id": "uuid|null",
  "target_trace_id": "uuid|null",
  "autoplay": "bool|null"
}
```

### New Analysis
```json
{
  "date": "YYYY-MM-DD|null",
  "user_id": "uuid"
}
```

### New Message
```json
{
  "recipient_user_id": "uuid",
  "landscape_analysis_id": "uuid|null",
  "trace_id": "uuid|null",
  "post_id": "uuid|null",
  "message_type": "general|question|mentor_reply|mentor_feedback|tarot_reading_request|null",
  "title": "string|null",
  "content": "string",
  "attachment_type": "string|null",
  "attachment": "json|null"
}
```

### New Post Message
```json
{
  "recipient_user_id": "uuid|null",
  "title": "string|null",
  "content": "string",
  "attachment_type": "string|null",
  "attachment": "json|null"
}
```

### New Journal Grant
```json
{
  "grantee_user_id": "uuid|null",
  "grantee_scope": "all_accepted_followers|all_platform_users|null",
  "access_level": "READ|null"
}
```

### New Post Grant
```json
{
  "grantee_user_id": "uuid|null",
  "grantee_scope": "all_accepted_followers|all_platform_users|null",
  "access_level": "READ|null"
}
```

### User Secure Action Create
```json
{
  "action_type": "password_reset",
  "email": "string|null"
}
```

### User Secure Action Consume
```json
{
  "action_type": "password_reset",
  "token": "string",
  "new_password": "string|null"
}
```

## Public Routes

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/` | none | `"Ok"` |
| POST | `/users` | `NewUser` | `User` |
| GET | `/sessions` | none | `Session` |
| POST | `/sessions` | login payload | `Session` |
| DELETE | `/sessions` | none | revoked `Session` |
| POST | `/user_secure_actions` | create secure action | `{ "message": "..." }` |
| POST | `/user_secure_actions/consume` | consume secure action | `{ "session": Session }` |

## Authenticated Routes

### Users

| Method | Path | Notes |
|---|---|---|
| GET | `/users` | Paginated user list |
| GET | `/users/search?q=...` | User search |
| GET | `/users/:id` | Self response includes `roles` |
| PUT | `/users/:id` | Update current user |
| GET | `/users/:id/posts` | Published posts visible to viewer |
| POST | `/users/:id/analysis` | Create analysis |
| GET | `/users/:id/analysis` | Last analysis |
| GET | `/users/:id/lens` | User lenses |
| GET | `/users/:id/traces` | User traces |
| GET | `/users/:id/journals` | User journals |
| GET | `/users/:id/heatmaps` | User heatmap |

### Mentors

| Method | Path | Notes |
|---|---|---|
| GET | `/mentors` | Mentor users |

### Admin

| Method | Path | Notes |
|---|---|---|
| GET | `/admin/recent_activity` | Admin only |
| GET | `/admin/service_users` | Admin only |
| POST | `/admin/service_users` | Admin only |
| GET | `/admin/service_users/:id` | Admin only |
| PUT | `/admin/service_users/:id` | Admin only |

### Drafts

| Method | Path | Notes |
|---|---|---|
| GET | `/drafts` | Current user non-empty draft `USER_TRACE`s, ordered by `updated_at desc` |

### Traces

| Method | Path | Notes |
|---|---|---|
| GET | `/traces/:id` | Owner-only |
| PUT | `/traces/:id` | Full update |
| PATCH | `/traces/:id` | Partial update |
| GET | `/traces/:id/posts` | Owner-only, paginated related posts |
| POST | `/traces/:id/posts` | Owner-only create related post |
| GET | `/traces/:id/analysis` | Returns analysis or `null` with `200` |
| GET | `/traces/:id/messages` | Owner-only trace conversation |
| POST | `/traces/:id/messages` | Owner-only trace conversation |

### Posts

| Method | Path | Notes |
|---|---|---|
| GET | `/posts` | Paginated shared feed |
| POST | `/posts` | Create post |
| GET | `/posts/:id` | Owner can see any status, others only published + granted |
| PUT | `/posts/:id` | Update post |
| GET | `/posts/:id/messages` | Post conversation |
| POST | `/posts/:id/messages` | Post conversation |
| GET | `/posts/:id/grants` | Owner-only |
| POST | `/posts/:id/grants` | Owner-only |
| DELETE | `/posts/:post_id/grants/:grant_id` | Owner-only revoke |
| GET | `/posts/users/:id` | Same as `/users/:id/posts` |

**Posts filters**
- `interaction_type` repeated
- `post_type` repeated
- `resource_type`
- `is_external`
- `user_id`
- `status`
- `limit`

Examples:
```http
GET /posts?interaction_type=inpt&interaction_type=outp
GET /posts?post_type=idea&post_type=book
GET /posts?status=published
```

Notes:
- `GET /posts` defaults to `status=published`
- post visibility uses post grants
- if a viewer can see both a broad and a specific post for the same `source_trace_id`, the more specific one wins

### Journals

| Method | Path | Notes |
|---|---|---|
| POST | `/journals` | Create journal |
| GET | `/journals/shared` | Journals readable through grants |
| GET | `/journals/shared/recent` | Ordered by latest visible shared post for current user |
| GET | `/journals/:id/draft` | Current draft trace for journal |
| POST | `/journals/:id/draft` | Create or get draft trace |
| GET | `/journals/:id` | Read journal |
| PUT | `/journals/:id` | Update journal |
| POST | `/journals/:id/exports` | Export journal |
| GET | `/journals/:id/grants` | List grants |
| POST | `/journals/:id/grants` | Create/reactivate grant |
| DELETE | `/journals/:journal_id/grants/:grant_id` | Revoke grant |
| GET | `/journals/:id/posts` | Posts related to the journal and visible to viewer |
| GET | `/journals/:id/traces` | Journal traces |
| POST | `/journals/:id/imports` | Import content into journal |

**Journal grant rules**
- Exactly one of `grantee_user_id` or `grantee_scope` must be set
- Direct user grants are limited to accepted followers
- `all_platform_users` is admin-only
- Creating/reactivating a direct user journal grant sends one journal-level email

### Relationships

| Method | Path | Notes |
|---|---|---|
| GET | `/relationships/followers` | Paginated followers |
| GET | `/relationships/following` | Paginated following |
| GET | `/relationships/requests/incoming` | Paginated incoming requests |
| GET | `/relationships/requests/outgoing` | Paginated outgoing requests |
| GET | `/relationships` | Relationship list |
| POST | `/relationships` | Create relationship/request |
| PUT | `/relationships/:id` | Update relationship |

### Transcriptions

| Method | Path | Notes |
|---|---|---|
| POST | `/transcriptions` | Multipart audio upload |

### Analysis

| Method | Path | Notes |
|---|---|---|
| POST | `/analysis` | Create analysis |
| GET | `/analysis/:id` | Get analysis |
| DELETE | `/analysis/:id` | Delete analysis |
| GET | `/analysis/:id/summaries` | Analysis summaries |
| POST | `/analysis/:id/summaries` | Create summary |
| GET | `/analysis/:id/landmarks` | Landmarks with sort options |
| GET | `/analysis/:id/elements` | Analysis elements |
| GET | `/analysis/:id/traces` | Analysis traces |
| GET | `/analysis/:id/trace_mirrors` | Analysis trace mirrors |
| GET | `/analysis/:id/parents` | Parent analyses |
| GET | `/analysis/:id/messages` | Analysis messages |
| GET | `/analysis/:id/feedback` | Latest mentor feedback or `null` |
| GET | `/analysis/:id/llm_calls` | Paginated analysis LLM calls |

**Analysis landmarks query params**
- `kind=all|mentioned|context`
- `order_by=related_elements_count|last_related_element_at|created_at`
- `order=asc|desc`

Default sort:
- `order_by=related_elements_count`
- `order=desc`

### Analysis Summaries

| Method | Path | Notes |
|---|---|---|
| GET | `/analysis_summaries/:id` | Read summary |
| PUT | `/analysis_summaries/:id` | Update summary |

### Elements

| Method | Path | Notes |
|---|---|---|
| GET | `/elements` | Paginated filtered elements |
| GET | `/elements/:id/landmarks` | Element landmarks |
| GET | `/elements/:id/relations` | Element relations |

**Elements filters**
- `element_type` repeated
- `element_subtype` repeated
- `status` repeated
- `interaction_date_from`
- `interaction_date_to`
- `created_at_from`
- `created_at_to`

Example:
```http
GET /elements?element_type=transaction&status=intended
GET /elements?status=in_progress&status=intended
```

### Landmarks

| Method | Path | Notes |
|---|---|---|
| GET | `/landmarks/:id` | Landmark detail |

### Lens

| Method | Path | Notes |
|---|---|---|
| POST | `/lens` | Create lens |
| GET | `/lens/:id/analysis` | Lens analyses |
| GET | `/lens/:id/aggregates/week_events` | Weekly aggregates |
| POST | `/lens/:id/retry` | Retry lens processing |
| PUT | `/lens/:id` | Update lens |
| DELETE | `/lens/:id` | Delete lens |

**Lens analysis filters**
- `landscape_analysis_type` repeated

Example:
```http
GET /lens/:id/analysis?landscape_analysis_type=daily_recap&landscape_analysis_type=weekly_recap
```

### LLM Calls

| Method | Path | Notes |
|---|---|---|
| GET | `/llm_calls` | Paginated current-user LLM calls |
| GET | `/llm_calls/:id` | Single LLM call |

**LLM call filters**
- `created_at_from`
- `created_at_to`

### Messages

| Method | Path | Notes |
|---|---|---|
| GET | `/messages` | Paginated current-user messages |
| POST | `/messages` | Create message |
| GET | `/messages/:id` | Read one message |
| PUT | `/messages/:id` | Update message |
| PATCH | `/messages/:id/seen` | Mark as seen |

**Message query params**
- `landscape_analysis_id`
- `received_only`
- `unread_only`
- pagination

### Trace Mirrors

| Method | Path | Notes |
|---|---|---|
| GET | `/trace_mirrors` | Current user trace mirrors |
| GET | `/trace_mirrors/:id` | Trace mirror detail |
| GET | `/trace_mirrors/:id/references` | Trace mirror references |
| GET | `/trace_mirrors/landscape/:landscape_id` | By landscape |
| GET | `/trace_mirrors/trace/:trace_id` | By trace |

## Current Sharing / Publication Semantics

- Finalizing a trace in a shared journal creates one default draft post server-side when needed
- Posts are the sharing objects; shared readers should consume posts, not raw traces
- `audience_role`:
  - `default`
  - `restricted`
- Default posts inherit journal grants when created
- Manual post-grant editing is blocked on `default` posts
- Post publication notifications are sent on post publication, not trace finalization

## Enum Style Notes

- `grantee_scope` is lowercase in API payloads:
  - `all_accepted_followers`
  - `all_platform_users`
- grant `access_level` and `status` remain uppercase in API payloads:
  - `READ`
  - `ACTIVE`
  - `REVOKED`

## Removed / Legacy Notes

This quick doc no longer documents the old `/resources`, `/interactions`, `/thought_input_usages`, or `/link_preview` API shapes because they are not part of the current router contract reflected here.
