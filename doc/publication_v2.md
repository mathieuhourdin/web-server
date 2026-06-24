Publication is a central part of hupo. Users share personal journal content, so the platform must make publication explicit, reviewable, and technically predictable.

Users should be able to:
- Modulate access grants with fine granularity.
- Trust that announced access rules are enforced by the backend.
- Avoid publishing or sharing content by lack of attention.

## Records

User-facing records are:
- Traces: pieces of journal writing, attached to a journal and a time of writing.
- Documents: static documents independent from a journal flow. They can be attached to or mentioned from traces.
- Albums: collections of traces, and later potentially collections of other records.

Posts are technical publication records. They connect a user-facing record to sharing, publication state, feed visibility, and access grants.

## Access Control

Access to records is calculated from posts and post grants.

Rules:
- A record may have zero or one post.
- A post has exactly one source record.
- A post source can be a trace, a document, or an album.
- Readers receive access through active `post_grants` on readable posts.
- Journal sharing policies are not direct access-control records.
- Journal sharing policies only help materialize default `post_grants`.

Owners always have stronger access than readers. Owner routes may expose full owner fields, while reader routes can return restricted projections.

## Post Lifecycle

Posts are not created automatically for every trace.

A trace-backed post is created explicitly, usually through:

```http
PUT /traces/{id}/post
```

This means the user has entered publication or sharing configuration for this trace.

Trace finalization does not create a post.
Publishing a post does not materialize journal policies again.

## Source Record Status

Record statuses define whether publication is allowed. Status cascade is negative and source-to-post only:
- Archiving a source record archives its post.
- Unarchiving or reactivating a source record never republishes the post automatically.
- Updating a post never updates the source record.
- Publishing a post is rejected if the source record is in a forbidden state.

### Traces

- `draft`: may have a draft post if the user explicitly prepared publication or sharing.
- `finalized`: may have a draft or published post. Published posts are allowed only when the trace is finalized.
- `archived`: only archived posts or no post.

Draft trace posts can have `post_grants` for preparation, but they are not readable by followers until the post becomes published.

### Documents

- `active`: published posts are allowed.
- `archived`: only archived posts or no post.

### Albums

- `complete` / `in_progress`: published posts are allowed.
- `archived`: only archived posts or no post.

Album visibility should eventually be represented through posts and post status rather than a separate album visibility field.

## Grants

`post_grants` are the source of truth for reader access.

Archiving a post does not delete or mutate its grants. Grants become inert because the post is no longer readable.

Re-publishing a post can make existing active grants effective again, subject to source record validity.

## Journal Sharing Policies

`journal_sharing_policies` are owner-managed defaults for a specific journal and grantee.

They are not used directly to calculate access.

Fields:
- `status`: lifecycle of the policy: `suggested`, `active`, `disabled`, `dismissed`, `revoked`.
- `default_future_access_enabled`: whether future explicit draft posts should receive a default grant for this grantee.
- `history_review_state`: whether historical propagation has been reviewed.
- `history_decision`: the decision made for historical propagation, if reviewed.
- `history_reviewed_at`: when historical propagation was reviewed.

Policy status and future default behavior are separate:
- `active` + `default_future_access_enabled = true`: future explicit posts receive a default grant.
- `active` + `default_future_access_enabled = false`: the policy exists, but future posts are not auto-granted.
- `suggested`: the owner still needs to review the policy.
- `dismissed` / `disabled` / `revoked`: the policy is not used for default future grants.

## Materialization

Journal policies are materialized into direct `post_grants` only at explicit moments:
- When an explicit trace post is first created.
- When the owner applies a history decision.

After materialization, `post_grants` are independent and become the source of truth for that post.

Policy updates do not resync existing post grants.
Publishing a post does not resync journal policies.
Trace finalization does not resync journal policies.

Default future materialization includes sensitive traces. The owner is in control and can remove grants before publication.

## History Review

When a policy is created or suggested, the owner may need to review historical propagation for this grantee and journal.

History review is explicit. No historical post is granted until the owner applies a history decision.

History decisions:
- `none`: mark history reviewed, grant nothing.
- `all_normal`: grant eligible history except sensitive traces.
- `all_including_sensitive`: grant all eligible history including sensitive traces.
- `user_selected`: grant exactly selected eligible posts, regardless of sensitivity.

Eligible history posts:
- Belong to the policy journal through a trace source.
- Are draft or published, not archived.
- Have a source trace that is a user trace.
- Have a source trace that is draft or finalized, not archived.

Invalid selected posts should return an error rather than being silently ignored.

## Journal Sharing Mode

Journal `sharing_mode` controls policy creation and suggestions, not direct access.

- `shared`: accepted followers get active policies with future defaults enabled.
- `semi_shared`: accepted followers get suggested policies for owner review.
- `private`: accepted followers do not get automatic policy suggestions.

When a follower is accepted, policies may be created depending on the journal sharing mode.
When a journal sharing mode changes, missing policies may be created for existing accepted followers.

Existing post grants are not rewritten by sharing mode changes.

## Pending Reviews

Owners can fetch policy reviews that need action.

Pending reviews include:
- Suggested policies that need future-default review.
- Active policies whose history review is not completed.

The frontend should display future-default review and history review as separate decisions, even when they appear on the same pending policy.

## Public Share Links

Public share links should use trace title and trace content for trace-backed views.

Public share link filtering is separate from authenticated `post_grants` access and should be conservative:
- Exclude sensitive traces unless explicitly allowed by the share-link design.
- Exclude records whose source or post state is archived.
- Avoid leaking owner-only fields.

## Attachments And Linked Documents

Access to a trace can imply access to attached documents when the document is needed to render or understand the trace.

This derived access should be handled by explicit backend policy, not by duplicating posts for every attachment.

Open question:
- How far should access propagate through general references, links, and mentions between traces and documents?
