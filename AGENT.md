# Hupo Backend Guidelines

`project_log.md` contains product and architecture notes. `object_design.md` contains broader
object-model decisions. This file contains implementation rules for backend changes.

## API and database conventions

- REST JSON fields use `snake_case`.
- REST enum values use lowercase `snake_case` via `#[serde(rename_all = "snake_case")]`.
  A one-word value is therefore lowercase, for example `blocked`.
- Database enum-like text values use `SCREAMING_SNAKE_CASE`.
- Rust enums persisted as text must have explicit `to_db()` and `from_db()` mappings. Serde names
  must not determine database storage values.
- Paginated endpoints return `PaginatedResponse<T>` with `items`, `limit`, `offset`, and `total`.
- Prefer additive response changes. Preserve compatibility fields during a client migration and
  remove them only deliberately.
- `PATCH` is for partial updates. Use `Option<Option<T>>` only when JSON `null` has a meaning
  distinct from an omitted field.

## Authorization and response projections

- Authorization is server-side and resource-based. Never trust client claims about ownership,
  visibility, grants, or roles.
- Reuse canonical access helpers such as `PostGrant::user_can_read_post`,
  `JournalSharingPolicy::user_can_read_journal`, and `Asset::can_user_read`; do not recreate
  authorization queries in route handlers.
- When a route has owner and reader views, use separate allow-list DTOs and choose the response
  variant in the handler. `UserResponse` is the established pattern.
- Do not create a reader response by serializing an owner response and manually clearing fields.
- Do not return asset URLs or metadata unless the caller can read the resource that uses the asset.
- List endpoints must avoid N+1 queries. Use joins or explicit batched hydration maps.
- User collection responses that include a profile picture must include
  `profile_picture_display_url`, preferring the public profile-picture asset URL and falling back
  to the legacy `profile_picture_url`.

## Content, sharing, and lifecycle

- Posts are technical publication and access-control records. They link a trace, document, or
  album; do not add new user-facing persisted post content.
- User-facing content is hydrated from its source record: trace title/content for trace posts,
  document data for document posts, and album data for album posts.
- Feed endpoints return `FeedItem` projections rather than raw post content.
- Journal sharing policies define defaults. Post grants are materialized access records. Do not
  rematerialize grants as an unrelated trace or post lifecycle side effect.
- Versioned trace mutations must require `expected_version` and return the resulting
  `version_integer`.
- Store image dimensions during upload. Hydrate them in batches for list responses.

## Persistence and code organization

- Add a new migration for schema changes. Do not edit migrations that may already have run.
- Keep domain code in `entities_v2` by concern: `records`, `social`, `derived_context`,
  `analysis_orchestration`, and `platform_infra`.
- Keep route handlers thin: parse input, authenticate/authorize, invoke domain code, and return a
  DTO. Put reusable persistence and hydration logic in the domain module.
- Use explicit request and response DTOs whenever persistence fields and API fields differ or a
  response depends on caller identity.

## Before adding a route

- Define the actor, target resource, and canonical access predicate.
- Define the response DTO for each viewer role.
- Define pagination and ordering for collection routes.
- Define idempotency and conflict behavior for mutations.
- Update the relevant API contract documentation.

## Verification

- Run `cargo fmt --check`, `cargo check`, and `cargo test --lib` after backend changes.
- Preserve unrelated worktree changes; never revert or rewrite them without explicit approval.
