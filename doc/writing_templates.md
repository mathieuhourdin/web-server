# Writing templates

Platform-managed writing starters, implemented in
`platform_infra/writing_template`. Templates are not user-generated records and
do not introduce AI calls, sharing grants, notifications or background jobs.

## Catalogue

All routes require an authenticated session.

- `GET /writing_templates?locale=fr&limit=100&offset=0`: active templates only.
- `GET /writing_templates/:id`: a template by UUID, including inactive templates
  so existing trace provenance can still be understood. Missing IDs return 404.

Collections use the existing `{items, total, limit, offset}` envelope. Pagination
defaults to 20, maximum 100. Order is ascending `sort_order`, then `slug`, then
`id`. Omit `locale` to retrieve all locales; provide one for an exact match after
normalization to lowercase (`fr-CA` becomes `fr-ca`). No automatic translation or
locale fallback. Public collection requests with `is_active=false` return 400.

```json
{
  "id": "11111111-1111-4111-8111-111111111111",
  "slug": "daily_scene",
  "title": "La scène du jour",
  "description": "Un moment précis de la journée que vous voulez garder.",
  "content": "Un moment précis que je veux garder :\n\nOù j’étais :\nAvec qui :\nCe qui s’est passé :\nCe que j’ai envie d’en retenir :",
  "locale": "fr",
  "sort_order": 20,
  "is_active": true,
  "created_at": "2026-11-04T14:00:00",
  "updated_at": "2026-11-04T14:00:00"
}
```

The 21 existing French mobile templates are seeded by migration, keeping their
current IDs as stable `slug` values. The new `id` is a database UUID. Existing
traces are left unassociated; matching content is not evidence of template use.

## Administration

An admin role is required, otherwise 403.

- `GET /admin/writing_templates`: active and inactive templates, same pagination
  and locale filter. Optional `is_active=true|false` filters the catalogue.
- `POST /admin/writing_templates`: create; returns 201 and the template.
- `PATCH /admin/writing_templates/:id`: edit; returns 200 and the template.

Create body:

```json
{
  "slug": "daily_scene",
  "title": "La scène du jour",
  "description": "Un moment à garder.",
  "content": "Ce que je veux garder :\n\n",
  "locale": "fr",
  "sort_order": 20,
  "is_active": true
}
```

Required: `slug`, `title`, `content`. Defaults: empty description, locale `fr`,
order 0, active true. Slugs are immutable, unique, 1–80 ASCII characters, starting
with a lowercase letter, then lowercase letters/digits/underscores/hyphens.
Titles are nonblank, at most 200 characters; descriptions at most 2000; content
nonblank, at most 20000 characters. Content line breaks/spacing are preserved.
Duplicate slugs return the existing database conflict response (409).

PATCH accepts any subset of `title`, `description`, `content`, `locale`,
`sort_order`, `is_active`. Unknown fields are rejected. To retire a template,
send `{"is_active":false}`. There is no hard-delete API; referenced templates
are also protected against database deletion by a restrictive foreign key.

## Trace provenance

`writing_template_id` is nullable provenance: the most recently applied template
while drafting. It is not a live content relationship or a complete usage log.

Supported by existing routes:

- `POST /journals/:id/draft`: optional template UUID, alongside ordinary content.
- `POST /journals/:id/traces/finalized`: optional template UUID on directly finalized creation.
- `PATCH /journals/:id/draft`, `PATCH /traces/:id`, `PUT /traces/:id`: use the
  normal required `expected_version_integer` and optional template field.

```json
{
  "expected_version_integer": 4,
  "writing_template_id": "11111111-1111-4111-8111-111111111111",
  "title": "La scène du jour",
  "content": "My own writing, based on the selected template."
}
```

On PATCH/PUT, absent means preserve, null means clear, UUID means set/replace.
New assignments must reference an active template; nonexistent/inactive choices
return 400 without saving any content. Existing associations remain valid when
deactivated, including on ordinary content edits and finalization. Changing or
clearing the association after finalization/archive is rejected with 400;
unchanged values remain acceptable. Only ordinary user/workspace writing can
receive templates. Existing trace ownership and optimistic-concurrency checks
are unchanged; stale saves return 409 without changing content or provenance.

The client still previews and applies the chosen title/body in the editor,
confirms replacement when needed, and saves the template UUID alongside the
resulting text. The server never silently replaces text from a UUID alone.
Subsequent template edits never rewrite traces. Applying a second template
replaces provenance; no many-to-many relation, template revisions or snapshots.

The nullable ID is returned to owners and readers in trace details/drafts,
journal/trace collections, feed trace items, album trace items and public
share-link trace objects. Non-trace feed items carry null. It grants no access
to any trace or journal, and catalogue access remains authenticated.

## Delivery

Migration `2026-11-04-140000_writing_templates` creates/seeds the catalogue and adds
the nullable trace FK/index. Startup applies it normally; no new environment
variables are required. Downgrade refuses to remove the field if any trace is
associated. Mobile catalogue replacement and an admin editing UI are separate
client work; existing mobile builds keep working with their hardcoded templates.
