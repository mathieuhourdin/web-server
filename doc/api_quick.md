# Quick API Documentation (Current Router)

Source of truth: `src/router.rs` and current handler signatures.

## Conventions

- Base URL: server root (examples assume `/` prefix).
- Auth:
  - `Required`: route is behind `auth_middleware_custom` (session user required).
  - `Public`: no auth middleware on the route.
  - Current session token format: `Authorization: Bearer <session_id>.<secret>`.
- Common query pagination: `offset` (optional, default `0`), `limit` (optional, default `20`).
- UUID path params are shown as `:id`, `:user_id`, etc.

## Main Payload Shapes

### `NewUser` (POST/PUT user)
```json
{
  "email": "string",
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
  "journal_theme": "Classic|White|Flowers|Dark|null",
  "current_lens_id": "uuid|null"
}
```

### `NewResource`
```json
{
  "title": "string",
  "subtitle": "string",
  "content": "string|null",
  "external_content_url": "string|null",
  "comment": "string|null",
  "image_url": "string|null",
  "resource_type": "4-char code|null",
  "maturing_state": "4-char code|null",
  "publishing_state": "4-char code|null",
  "is_external": "bool|null",
  "entity_type": "4-char code|null",
  "resource_subtype": "string|null"
}
```

### `NewInteraction`
```json
{
  "interaction_user_id": "uuid|null",
  "interaction_progress": "int",
  "interaction_comment": "string|null",
  "interaction_date": "datetime|null",
  "interaction_type": "4-char code|null",
  "interaction_is_public": "bool|null",
  "resource_id": "uuid|null"
}
```

### `NewTraceDto`
```json
{
  "content": "string",
  "journal_id": "uuid"
}
```

### `NewPostDto`
```json
{
  "title": "string",
  "subtitle": "string|null",
  "content": "string",
  "image_url": "string|null",
  "post_type": "outp|inpt|pblm|wish|null",
  "publishing_date": "datetime|null",
  "publishing_state": "string|null",
  "maturing_state": "4-char code|null"
}
```

### `NewJournalDto`
```json
{
  "title": "string",
  "subtitle": "string|null",
  "content": "string|null",
  "journal_type": "MetaJournal|WorkLogJournal|ReadingNoteJournal|null"
}
```

### `UpdateJournalDto`
```json
{
  "title": "string|null",
  "subtitle": "string|null",
  "content": "string|null",
  "journal_type": "MetaJournal|WorkLogJournal|ReadingNoteJournal|null",
  "status": "Draft|Published|Archived|null"
}
```

### `NewLensDto`
```json
{
  "processing_state": "OutOfSync|InSync|null",
  "fork_landscape_id": "uuid|null",
  "target_trace_id": "uuid|null",
  "autoplay": "bool|null"
}
```

### `NewAnalysisDto`
```json
{
  "date": "YYYY-MM-DD|null",
  "user_id": "uuid"
}
```

### `NewResourceRelation` (POST `/thought_input_usages`)
```json
{
  "origin_resource_id": "uuid",
  "target_resource_id": "uuid",
  "relation_comment": "string",
  "user_id": "uuid|null",
  "relation_type": "string|null",
  "relation_entity_pair": "SCREAMING_SNAKE_CASE|null",
  "relation_meaning": "SCREAMING_SNAKE_CASE|null"
}
```

### Session login payload
```json
{
  "username": "string",
  "password": "string"
}
```

### Link preview payload
```json
{
  "external_content_url": "string"
}
```

## Routes

### Public Routes

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/` | none | `"Ok"` |
| POST | `/users` | `NewUser` | `User` |
| GET | `/sessions` | none | `Session` |
| POST | `/sessions` | `{ username, password }` | `Session` (`token` contains bearer token) |
| DELETE | `/sessions` | none (uses current bearer token) | `Session` (revoked) |
| POST | `/link_preview` | `NewLinkPreview` | `LinkPreview` |

### Users (`/users`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/users` | query: `offset`, `limit` | `UserPseudonymizedResponse[]` |
| GET | `/users/:id` | path: `id` | `UserPseudonymizedResponse` or `UserPseudonymizedAuthentifiedResponse` |
| PUT | `/users/:id` | path: `id`, body: `NewUser` | `User` |
| GET | `/users/:id/posts` | path: `id`, query: `offset`, `limit` | `Post[]` |
| POST | `/users/:id/analysis` | path: `id`, body: `NewAnalysisDto` | `Resource` (analysis resource) |
| GET | `/users/:id/analysis` | path: `id` | `InteractionWithResource` (last analysis) |
| GET | `/users/:id/lens` | path: `id` | `Lens[]` |
| GET | `/users/:id/traces` | path: `id` | `Trace[]` |
| GET | `/users/:id/journals` | path: `id` | `Journal[]` |
| GET | `/users/:id/heatmaps` | path: `id`, query: `from`, `to` (YYYY-MM-DD) | `HeatmapRow[]` |

### Resources (`/resources`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/resources` | query: `is_external`, `resource_type`, `offset`, `limit` | `Resource[]` |
| POST | `/resources` | body: `NewResource` | `Resource` |
| GET | `/resources/:id` | path: `id` | `Resource` |
| PUT | `/resources/:id` | path: `id`, body: `NewResource` | `Resource` |
| GET | `/resources/:id/author_interaction` | path: `id` | `Interaction` |
| GET | `/resources/:id/interactions` | path: `id`, query: `offset`, `limit` | `InteractionWithResource[]` |
| POST | `/resources/:id/interactions` | path: `id`, body: `NewInteraction` | `Interaction` |
| GET | `/resources/:id/bibliography` | path: `id` | `ResourceRelationWithOriginResource[]` |
| GET | `/resources/:id/usages` | path: `id` | `ResourceRelationWithTargetResource[]` |

### Traces (`/traces`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| POST | `/traces` | body: `NewTraceDto` | `Resource` |
| GET | `/traces/:id` | path: `id` | `Trace` |
| GET | `/traces/:id/analysis` | path: `id` | `LandscapeAnalysis` (filtered by current lens scope) |

### Posts (`/posts`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/posts` | query: `post_type`, `resource_type`, `is_external`, `user_id`, `maturing_state`, `limit` | `Post[]` |
| POST | `/posts` | body: `NewPostDto` | `Post` |
| GET | `/posts/:id` | path: `id` (`interaction.id`) | `Post` |
| PUT | `/posts/:id` | path: `id`, body: `NewPostDto` | `Post` |
| GET | `/posts/:id/bibliography` | path: `id` | `ResourceRelationWithOriginResource[]` |
| GET | `/posts/:id/usages` | path: `id` | `ResourceRelationWithTargetResource[]` |
| GET | `/posts/users/:id` | path: `id`, query: `offset`, `limit` | `Post[]` |

### Journals (`/journals`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| POST | `/journals` | body: `NewJournalDto` (`title` required) | `Journal` |
| PUT | `/journals/:id` | path: `id`, body: `UpdateJournalDto` | `Journal` |
| GET | `/journals/:id/traces` | path: `id` | `Trace[]` |
| POST | `/journals/:id/import_text` | path: `id`, multipart file (`file` preferred) | `ImportJournalResult` |

### Interactions (`/interactions`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/interactions` | query: `interaction_type`, `maturing_state`, `interaction_user_id`, `resource_type`, `offset`, `limit` | `InteractionWithResource[]` |
| PUT | `/interactions/:id` | path: `id`, body: `NewInteraction` | `Interaction` |

### Resource Relations - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| POST | `/thought_input_usages` | body: `NewResourceRelation` | `ResourceRelation` |

### Transcriptions (`/transcriptions`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| POST | `/transcriptions` | multipart audio file | `{ "content": "..." }` |

### Analysis (`/analysis`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| POST | `/analysis` | body: `NewAnalysisDto` | `Resource` |
| GET | `/analysis/:id` | path: `id` | `LandscapeAnalysis` |
| DELETE | `/analysis/:id` | path: `id` | `LandscapeAnalysis` |
| GET | `/analysis/:id/landmarks` | path: `id`, query: `kind=all|mentioned|context` | `Landmark[]` |
| GET | `/analysis/:id/elements` | path: `id` | `Element[]` |
| GET | `/analysis/:id/parents` | path: `id` | `LandscapeAnalysis[]` |
| GET | `/analysis/:id/llm_calls` | path: `id` | `LlmCall[]` |

### Landmarks (`/landmarks`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/landmarks/:id` | path: `id` | `LandmarkWithParentsAndElements` |

### Elements (`/elements`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/elements/:id/landmarks` | path: `id` | `Landmark[]` |
| GET | `/elements/:id/relations` | path: `id` | `ElementRelationWithRelatedElement[]` |

### Lens (`/lens`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| POST | `/lens` | body: `NewLensDto` | `Lens` |
| GET | `/lens/:id/analysis` | path: `id` | `LandscapeAnalysis[]` |
| PUT | `/lens/:id` | path: `id`, body: `NewLensDto` | `Lens` |
| DELETE | `/lens/:id` | path: `id` | `Lens` |

### LLM Calls (`/llm_calls`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/llm_calls` | query: `offset`, `limit` | `LlmCall[]` |
| GET | `/llm_calls/:id` | path: `id` | `LlmCall` |

### Trace Mirrors (`/trace_mirrors`) - Auth Required

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/trace_mirrors` | none | `TraceMirror[]` |
| GET | `/trace_mirrors/:id` | path: `id` | `TraceMirror` |
| GET | `/trace_mirrors/:id/references` | path: `id` | `Reference[]` |
| GET | `/trace_mirrors/landscape/:landscape_id` | path: `landscape_id` | `TraceMirror[]` |
| GET | `/trace_mirrors/trace/:trace_id` | path: `trace_id` | `TraceMirror[]` |

---

## Notes

- This document is generated manually from code (no OpenAPI tooling).
- Some response unions are runtime-dependent (`/users/:id`).
- Legacy naming remains in one route: `POST /thought_input_usages` (resource relations create).
