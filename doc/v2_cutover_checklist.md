# V2 Migration Checklist (Non-Prod, 2 Steps)

Scope assumptions:

- No concurrent writes during migration window.
- Some row loss is acceptable if needed.
- Fast rollback strategy is: keep legacy tables untouched until step 2, then rerun step 1 if needed.

---

## Migration Strategy

### Step 1: Create v2 tables, backfill, switch runtime reads/writes to v2

Goal: app works only on v2 tables while legacy tables still exist.

### Step 2: Drop legacy tables

Goal: remove `resources`, `resource_relations`, `interactions` and legacy code paths.

---

## Target Tables To Create (Step 1)

Entity tables:

- `journals`
- `traces`
- `trace_mirrors`
- `elements`
- `landmarks`
- `references`
- `landscape_analyses`
- `lenses`
- `posts`

Relation tables:

- `element_relations` (element -> element: applies_to, subtask_of, theme_of)
- `element_landmarks` (element -> landmark: involves)
- `landmark_relations` (landmark -> landmark: child_of, high_level_project_related_to)
- `landscape_landmarks` (landscape_analysis -> landmark: owned_by_analysis, referenced)
- `lens_analysis_scopes` (lens -> landscape_analysis: includes_in_scope)
- `lens_heads` (lens -> landscape_analysis: has_current_head)
- `lens_targets` (lens -> trace: targets_trace)
- `post_relations` (post -> post: bibliography)

Notes:

- `traces.journal_id`, `traces.user_id`, `trace_mirrors.trace_id`, `trace_mirrors.landscape_analysis_id`, `elements.trace_id`, `elements.trace_mirror_id`, `elements.analysis_id`, `landmarks.analysis_id`, and `references.trace_mirror_id/landmark_id` are modeled as direct FK columns (not separate relation tables).
- `trace_mirrors.primary_landmark_id` is modeled as a nullable FK column on `trace_mirrors` (not a separate relation table).
- `landscape_analyses.parent_id` and `landscape_analyses.replayed_from_id` are modeled as columns on `landscape_analyses`.

## Proposed Fields Per Table

Entity tables:

- `journals`
  - `id UUID PK`
  - `user_id UUID NOT NULL`
  - `title TEXT NOT NULL`
  - `subtitle TEXT NOT NULL`
  - `content TEXT NOT NULL`
  - `journal_type TEXT NOT NULL` (`META_JOURNAL | WORK_LOG_JOURNAL | READING_NOTE_JOURNAL`)
  - `status TEXT NOT NULL` (`DRAFT | PUBLISHED | ARCHIVED`)
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `traces`
  - `id UUID PK`
  - `user_id UUID NOT NULL`
  - `journal_id UUID NOT NULL FK -> journals(id)`
  - `title TEXT NOT NULL`
  - `subtitle TEXT NOT NULL`
  - `content TEXT NOT NULL`
  - `interaction_date TIMESTAMP NULL`
  - `trace_type TEXT NOT NULL` (`USER_TRACE | BIO_TRACE | WORKSPACE_TRACE | HIGH_LEVEL_PROJECTS_DEFINITION`)
  - `status TEXT NOT NULL` (`DRAFT | FINALIZED | ARCHIVED`)
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `trace_mirrors`
  - `id UUID PK`
  - `user_id UUID NOT NULL`
  - `trace_id UUID NOT NULL FK -> traces(id)`
  - `landscape_analysis_id UUID NOT NULL FK -> landscape_analyses(id)`
  - `primary_landmark_id UUID NULL FK -> landmarks(id)`
  - `title TEXT NOT NULL`
  - `subtitle TEXT NOT NULL`
  - `content TEXT NOT NULL`
  - `trace_mirror_type TEXT NOT NULL` (`NOTE | JOURNAL | HIGH_LEVEL_PROJECTS | BIO`)
  - `tags JSONB NOT NULL` (default `[]`)
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `elements`
  - `id UUID PK`
  - `user_id UUID NOT NULL`
  - `analysis_id UUID NOT NULL FK -> landscape_analyses(id)`
  - `trace_id UUID NOT NULL FK -> traces(id)`
  - `trace_mirror_id UUID NULL FK -> trace_mirrors(id)`
  - `title TEXT NOT NULL`
  - `subtitle TEXT NOT NULL`
  - `content TEXT NOT NULL`
  - `extended_content TEXT NULL`
  - `verb TEXT NOT NULL`
  - `element_type TEXT NOT NULL` (`TRANSACTION | DESCRIPTIVE | NORMATIVE | EVALUATIVE`)
  - `element_subtype TEXT NOT NULL`
  - `interaction_date TIMESTAMP NULL`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `landmarks`
  - `id UUID PK`
  - `analysis_id UUID NOT NULL FK -> landscape_analyses(id)`
  - `user_id UUID NOT NULL`
  - `parent_id UUID NULL FK -> landmarks(id)`
  - `title TEXT NOT NULL`
  - `subtitle TEXT NOT NULL`
  - `content TEXT NOT NULL`
  - `external_content_url TEXT NULL`
  - `comment TEXT NULL`
  - `image_url TEXT NULL`
  - `landmark_type TEXT NOT NULL`
  - `maturing_state TEXT NOT NULL`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `references`
  - `id UUID PK`
  - `trace_mirror_id UUID NOT NULL FK -> trace_mirrors(id)`
  - `landmark_id UUID NULL FK -> landmarks(id)`
  - `landscape_analysis_id UUID NOT NULL FK -> landscape_analyses(id)`
  - `user_id UUID NOT NULL`
  - `tag_id INT NOT NULL`
  - `mention TEXT NOT NULL`
  - `reference_type TEXT NOT NULL`
  - `context_tags JSONB NOT NULL` (default `[]`)
  - `reference_variants JSONB NOT NULL` (default `[]`)
  - `parent_reference_id UUID NULL FK -> references(id)`
  - `is_user_specific BOOLEAN NOT NULL`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `landscape_analyses`
  - `id UUID PK`
  - `user_id UUID NOT NULL`
  - `title TEXT NOT NULL`
  - `subtitle TEXT NOT NULL`
  - `plain_text_state_summary TEXT NOT NULL`
  - `interaction_date TIMESTAMP NULL`
  - `processing_state TEXT NOT NULL` (`PENDING | REPLAY_REQUESTED | COMPLETED`)
  - `parent_id UUID NULL FK -> landscape_analyses(id)`
  - `replayed_from_id UUID NULL FK -> landscape_analyses(id)`
  - `analyzed_trace_id UUID NULL FK -> traces(id)`
  - `trace_mirror_id UUID NULL FK -> trace_mirrors(id)`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `lenses`
  - `id UUID PK`
  - `user_id UUID NOT NULL`
  - `processing_state TEXT NOT NULL` (`OUT_OF_SYNC | IN_SYNC`)
  - `fork_landscape_id UUID NULL FK -> landscape_analyses(id)`
  - `current_landscape_id UUID NULL FK -> landscape_analyses(id)`
  - `target_trace_id UUID NULL FK -> traces(id)`
  - `autoplay BOOLEAN NOT NULL`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `posts`
  - `id UUID PK`
  - `user_id UUID NOT NULL`
  - `title TEXT NOT NULL`
  - `subtitle TEXT NOT NULL`
  - `content TEXT NOT NULL`
  - `image_url TEXT NULL`
  - `post_type TEXT NOT NULL` (`OUTPUT | INPUT | PROBLEM | WISH`)
  - `publishing_date TIMESTAMP NULL`
  - `publishing_state TEXT NOT NULL`
  - `maturing_state TEXT NOT NULL`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

Relation tables:

- `element_relations`
  - `id UUID PK`
  - `origin_element_id UUID NOT NULL FK -> elements(id)`
  - `target_element_id UUID NOT NULL FK -> elements(id)`
  - `relation_type TEXT NOT NULL` (`APPLIES_TO | SUBTASK_OF | THEME_OF`)
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `element_landmarks`
  - `id UUID PK`
  - `element_id UUID NOT NULL FK -> elements(id)`
  - `landmark_id UUID NOT NULL FK -> landmarks(id)`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `landmark_relations`
  - `id UUID PK`
  - `origin_landmark_id UUID NOT NULL FK -> landmarks(id)`
  - `target_landmark_id UUID NOT NULL FK -> landmarks(id)`
  - `relation_type TEXT NOT NULL` (`CHILD_OF | HIGH_LEVEL_PROJECT_RELATED_TO`)
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `landscape_landmarks`
  - `id UUID PK`
  - `landscape_analysis_id UUID NOT NULL FK -> landscape_analyses(id)`
  - `landmark_id UUID NOT NULL FK -> landmarks(id)`
  - `relation_type TEXT NOT NULL` (`OWNED_BY_ANALYSIS | REFERENCED`)
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `lens_analysis_scopes`
  - `id UUID PK`
  - `lens_id UUID NOT NULL FK -> lenses(id)`
  - `landscape_analysis_id UUID NOT NULL FK -> landscape_analyses(id)`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `lens_heads`
  - `id UUID PK`
  - `lens_id UUID NOT NULL FK -> lenses(id)`
  - `landscape_analysis_id UUID NOT NULL FK -> landscape_analyses(id)`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `lens_targets`
  - `id UUID PK`
  - `lens_id UUID NOT NULL FK -> lenses(id)`
  - `trace_id UUID NOT NULL FK -> traces(id)`
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

- `post_relations`
  - `id UUID PK`
  - `origin_post_id UUID NOT NULL FK -> posts(id)`
  - `target_post_id UUID NOT NULL FK -> posts(id)`
  - `relation_type TEXT NOT NULL` (`BIBLIOGRAPHY`)
  - `created_at TIMESTAMP NOT NULL`
  - `updated_at TIMESTAMP NOT NULL`

---

## Step 1 Checklist

## 1) DB: create v2 schema

- [ ] Create dedicated tables for:
  - [ ] records: `journals`, `traces`
  - [ ] derived_context: `trace_mirrors`, `elements`, `landmarks`, `references`
  - [ ] analysis_orchestration: `landscape_analyses`, `lenses`
  - [ ] social: `posts`
- [ ] Create dedicated relation tables listed above.

## 2) DB: backfill from legacy tables

- [ ] Backfill all v2 entity tables from legacy rows.
- [ ] Backfill all v2 relation tables from `resource_relations`.
- [ ] Keep legacy tables unchanged.

## 3) Runtime cutover by module

### Records
- [ ] `src/entities_v2/records/journal/*` reads/writes v2 tables only.
- [ ] `src/entities_v2/records/trace/*` reads/writes v2 tables only.
- [ ] `src/entities_v2/records/journal_import/*` uses v2 ownership/checks.

### Derived Context
- [ ] `src/entities_v2/derived_context/trace_mirror/*` reads/writes v2 tables only.
- [ ] `src/entities_v2/derived_context/element/*` reads/writes v2 tables only.
- [ ] `src/entities_v2/derived_context/landmark/*` reads/writes v2 tables only.
- [ ] `src/entities_v2/derived_context/reference/*` reads/writes v2 tables only.

### Analysis Orchestration
- [ ] `src/entities_v2/analysis_orchestration/lens/*` reads/writes v2 tables only.
- [ ] `src/entities_v2/analysis_orchestration/landscape_analysis/*` reads/writes v2 tables only.

### Social
- [ ] `src/entities_v2/social/post/*` reads/writes v2 tables only.

### Work Analyzer
- [ ] `src/work_analyzer/*` no longer depends on legacy `NewResourceRelation/ResourceRelation/Interaction/Resource`.

## 4) Quick functional checks (minimal)

- [ ] Create/update/list journal works.
- [ ] Create/update/list trace works.
- [ ] Run one analysis pipeline end-to-end works.
- [ ] Fetch lens/analysis/trace_mirror/element/landmark/reference works.
- [ ] Post CRUD/list works.

## 5) Replay-ready status

- [ ] If checks fail: keep legacy tables, fix code/migrations, rerun step 1.

---

## Step 2 Checklist (Drop Legacy)

- [ ] Remove remaining imports/usages of:
  - `src/entities/resource*`
  - `src/entities/interaction*`
  - `src/entities/resource_relation.rs`
- [ ] Drop tables:
  - [ ] `interactions`
  - [ ] `resource_relations`
  - [ ] `resources`
- [ ] Run `cargo check` and minimal smoke checks again.

---

## Suggested execution order (inside step 1)

1. Records
2. Derived Context
3. Analysis Orchestration
4. Work Analyzer
5. Social

Reason: records and core context are prerequisites for analyzer and higher-level flows.
