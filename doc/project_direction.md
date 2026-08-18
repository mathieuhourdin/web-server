# Hupo Backend: Current Direction

**Status: August 2026.** This document is an orientation guide for the current backend. It
records the architectural direction that is not obvious from the router, migrations, historical
notes, or individual API contracts. It is not a substitute for `doc/api_quick.md`: the router and
DTOs remain the contract source of truth.

Related documents:
- `AGENT.md`: implementation and API conventions.
- `doc/publication_v2.md`: detailed publication and sharing semantics.
- `project_log.md`: chronological, exploratory notes. Older entries can describe models that have
  since changed.
- `doc/v2_cutover_checklist.md`: historical v2 migration plan, not a statement that every legacy
  concern is now resolved.

## Product Model

Hupo is a personal journal and reflection platform. A trace is the central unit of writing. It can
be private, finalized, analysed, optionally published, and used as the context for conversations.
The platform also lets users manage documents, albums, social connections, mentor interactions,
and derived analytical context.

The model intentionally separates three concerns:

1. **Records** hold user material: journals, traces, documents, albums, attachments, and source
   assets.
2. **Social publication** controls who can discover and read a record: posts, post grants,
   relationships, feeds, messages, sharing policies, and share links.
3. **Derived context and analysis** extract a user's evolving landscape: landscape analyses,
   lenses, trace mirrors, elements, references, landmarks, and high-level projects.

Posts are important but are no longer the primary user-facing content object. They are technical
publication/access records pointing to a trace, document, or album. User-facing lists should
return hydrated projections from their source record.

## Current Record Lifecycle

### Journals and traces

Published `USER_JOURNAL` journals are intended to have exactly one persisted current draft trace.
The draft is the writing buffer; finalizing it creates a new blank draft for subsequent writing.
Meta journals are an explicit exception because they support biography and high-level-project
workflows rather than normal ongoing journal writing.

Traces are updated in place. The project deliberately removed `trace_versions`: there is no edit
history model at present. Trace mutations that change core trace fields use optimistic concurrency
through `expected_version` / `expected_version_integer` and return the resulting
`version_integer`. Attachment collections are intentionally outside this first concurrency scope.

Trace content, title, subtitle, and content image are the canonical presentation data. The former
model where posts could carry custom display content has been rejected. If material needs a
different audience-facing text, it should become a distinct trace rather than an override on a
post.

### Documents, albums, and attachments

Documents are independent records with a content source: persisted content, an internal asset, an
external URL, or reference-only metadata. Albums are curated collections that can be published in
the same way as traces and documents.

`trace_attachments` links traces to documents. Do not recreate raw asset attachments for normal
documents: document records provide a consistent model for internal files and external URLs.
Access to an attached document can be derived from access to the trace where it is needed for
rendering; it should not require duplicating a post.

`trace_source_assets` is separate from document attachments and `content_image_asset_id`. It is an
ordered, owner-private collection of image assets uploaded as source material for a trace, initially
for photographed handwritten journals. It is designed to become input to OCR or an LLM later. It
does not currently alter trace text, trigger analysis, or become visible to trace readers.

## Publication and Sharing

The detailed rules are in `doc/publication_v2.md`. The important boundary is:

- A post is explicit. Trace finalization does not create one automatically.
- A post can exist for a draft trace so the owner can prepare grants before finalization, but it is
  not readable until it is published and its source is eligible.
- `post_grants` are the source of truth for reader access.
- `journal_sharing_policies` are per-journal, per-grantee defaults and review workflow, not direct
  reader access grants.
- Policies are materialized into post grants only when an explicit trace post is created or a
  history decision is applied. They never resync existing grants as a side effect of publishing,
  finalization, or a policy edit.

Sharing modes only create policy workflow:
- `shared`: accepted followers can get active future-default policies.
- `private`: no automatic policy is created.
- `semi_shared` currently behaves like `private`; automatic suggestion policies were deliberately
  stopped until there is a clearer review experience.

History propagation is owner-directed and supports `none`, `all_normal`,
`all_including_sensitive`, and `user_selected`. Sensitive content is excluded only by the
`all_normal` choice, not by a hidden global rule.

Public share links are separate from authenticated grants and should remain conservative. Their
trace projection uses trace title/content and excludes content that the public-link policy does not
explicitly permit.

## Access-Control Direction

Authorization is resource based and server enforced. Ownership does not imply that a reader
should receive the owner DTO. Routes with materially different owner and reader data should use
separate allow-list DTOs selected in the handler, rather than serializing a full record then
clearing fields.

Asset access is centralized in `Asset::can_user_read`. An asset can be used by several records;
access is allowed when at least one readable usage permits it. Usage types are an internal
authorization mechanism, not a single persisted classification of the asset. Public profile
images are the exception: they are copied to the public bucket and exposed by a stable public URL.

All other assets remain private and are served through authorized signed URLs. Bucket prefixes are
currently generic. Semantic private prefixes for trace content images, documents, and source
imports are desirable for operations and lifecycle management, but they are deferred and must not
be treated as a security boundary.

## Read Models and APIs

`FeedItem` is the reading projection for social feed responses. It denormalizes title, content,
and cover image from the linked trace, document, or album while preserving the post's publication
metadata. `/feed` is the preferred endpoint; `/posts` remains temporarily for compatibility and
technical/post-specific workflows.

Journal trace collections return restricted reader projections and fuller owner projections. They
support unread state and positioning around an `until_trace_id`; the latter returns a batch aligned
to twenty items so a client can open a journal around a selected trace.

Seen state is persisted in `user_post_states`, because posts are the shared/readable unit. Both
`PUT /traces/:id/seen` and `POST /user_post_states` currently mark a post as seen. There is no
`PUT /posts/:id/seen` route at present. This is legacy API duplication to rationalize later, not a
new pattern to extend casually.

User profile pictures should be exposed to clients through `profile_picture_display_url`, preferring
the public profile-picture asset URL and falling back to the legacy stored URL. Collection DTOs
should not expose the legacy fields.

## Search and Derived Context

PostgreSQL full-text search is the current search implementation. It indexes finalized owner
traces and supports text search as well as journal, landmark, and high-level-project filters. No
query is also useful: it behaves as an anti-chronological filtered trace list.

Derived context in search is lens scoped. The relevant path is the trace incremental landscape
analysis through `lens_analysis_scopes`; derived context outside the user's current lens must not
be indexed or returned as if it belonged to that lens. The search document currently uses:

- trace title and content;
- reference `mention`, `context_tags`, and `reference_variants`;
- reference-derived non-high-level-project landmarks;
- reference-derived high-level-project landmarks.

It intentionally does not index trace-mirror content, element content, trace subtitles, or
landmark subtitles. Search documents refresh when trace analysis completes, rather than on every
derived-context write. A change of `current_lens_id` does not yet trigger a rebuild. Elasticsearch
is a later scalability option, not a current dependency.

The broader analytical model remains a lens-based incremental landscape: each landscape analysis
processes a trace relative to its predecessor and produces derived context. This is valuable but
still has cleanup and operational work before it should be treated as a fully stable subsystem.

## Identity, Devices, and Notifications

Sessions may be device-bound. A device is created during login when the client provides its locally
persisted identifier and can later update its push token. Browser clients may store a generated
UUID in local storage. Existing sessions without a device do not require forced relogin; a session
can be associated with a device through the supported device flow.

Email/password, Google, and Apple identity flows coexist. Provider subjects are stored on the user
for the current MVP. A provider login may match an existing user by verified provider email and
link the provider identity; it must not overwrite an existing password hash.

Push notifications use FCM. Message notifications are data-only and include sender identity,
message data, and the public profile picture URL where available. Published-post notifications are
independent from email digest preference. The current notification coordination is deliberately
non-persisted: notification delivery is best-effort and asynchronous. A future unified
notification service can decide push versus email and record delivery state, but should not be
introduced until product needs justify persistence and retry semantics.

## AI Safety and Moderation

AI use has two distinct controls: a user-controlled preference for AI features and an
administrator-controlled allowance. The latter is the hard safety switch. Server-side guards apply
global per-kind daily quotas, including mentor feedback, mentor questions, tarot readings,
transcription, and landscape analysis. Counts currently come from existing LLM-call records rather
than a dedicated quota table.

Content reports snapshot message or post content at reporting time, so later edits/deletions do not
erase the moderation evidence. Administration routes read those snapshots.

Demo accounts are intended to be invisible to normal users and restricted from selected social
actions. Any new social or notification path must preserve those restrictions.

## Deliberately Rejected or Deferred Approaches

- **Persisted trace versions:** rejected for now. There is one mutable trace record with optimistic
  concurrency, not a read/write version pair or trace-version API.
- **Automatic posts at finalization or for shared journals:** rejected. Publication is always an
  explicit owner action.
- **Post-specific content/title/image overrides:** rejected. Posts are technical publication
  records; source records supply presentation content.
- **Journal policies as direct grants or continuously synchronized defaults:** rejected. Policies
  materialize post grants at explicit times only.
- **Automatic semi-shared suggestions:** paused. `semi_shared` currently has private behavior.
- **Raw asset attachments in place of documents:** rejected for ordinary attachments. Documents
  are the attachment abstraction; `trace_source_assets` is a separate ingestion abstraction.
- **OCR/LLM transcription of handwritten photos:** deliberately deferred. The current source-asset
  relation is only the safe ingestion foundation. Later processing needs explicit jobs, extracted
  text/provenance, user review, and an apply-to-trace action guarded by trace concurrency.
- **Bucket-prefix reorganization:** deferred. New asset keys remain generic until lifecycle needs
  justify a controlled migration/new-upload policy.
- **Persistent unified notification queue:** deferred. Current push/email behavior is best effort.
- **Email verification:** not currently part of signup. Account creation was intentionally split
  into an initial email/password request followed by profile completion, without introducing an
  incomplete-account state as an authorization gate.

## Near-Term Cleanup Priorities

1. Reconcile stale quick documentation and legacy endpoint aliases with the router, especially old
   post and asset examples.
2. Continue response-projection cleanup: owner/reader DTO separation, no legacy profile fields in
   collection responses, and no accidental private asset metadata exposure.
3. Consolidate duplicate/legacy state-writing endpoints where clients have migrated, beginning
   with post seen-state semantics.
4. Add focused integration tests around access control, sharing-policy materialization, trace draft
   invariants, and optimistic concurrency. These are higher value than broad unit-test coverage.
5. Audit old `resources`, `resource_relations`, and `interactions` paths before dropping tables or
   claiming the v2 cutover is complete. They have been considered obsolete in product direction,
   but removal must be proven by code and migration audit.
6. Stabilize the landscape-analysis pipeline: job failure behavior, replay/rebuild semantics,
   current-lens rebuild behavior, and observability/cost controls.

## Likely Next Product Directions

- Handwritten journal ingestion: source-image upload is available; next comes user-approved OCR or
  LLM extraction into a trace draft.
- More complete feed/read projections and eventual deprecation of legacy post-list routes.
- Public web pages: static public content can remain served by Nginx; data-backed SSR pages should
  be an explicit server-rendering application or Axum route layer with the same authorization and
  domain services, not an accidental extension of the SPA API.
- Further document/album publication and asset-visibility refinement.
- Better analytical navigation around landmarks, high-level projects, and lenses once the
  incremental pipeline is operationally dependable.
