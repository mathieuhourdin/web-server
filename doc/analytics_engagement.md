# Engagement analytics and reading events

All analytics routes require a regular admin session. These are read-time queries,
not stored aggregates. Existing metric names/default selections are unchanged.

## Daily series

`GET /admin/analytics/platform_series?from=2026-10-01&to=2026-11-02&metric=daily_publishers&metric=daily_private_writers&metric=inactive_writers_30d&metric=inactive_writers_60d`

The existing response envelope is unchanged:

```json
{
  "from": "2026-10-01",
  "to": "2026-11-02",
  "series": [
    {"metric": "daily_publishers", "points": [{"date": "2026-10-01", "count": 12}]}
  ]
}
```

- `daily_publishers`: distinct authors with at least one currently shared trace
  first published on that UTC date. Counts their own trace-backed posts, including
  blog publications. Uses the earliest recorded distribution timestamp per trace,
  falling back to the post's publication/creation date for legacy records. Multiple
  posts, distribution events, recipients and notifications do not multiply counts;
  redistributing an existing trace does not move its original publication date.
- `daily_private_writers`: distinct authors finalizing at least one currently
  private trace on that UTC date (`finalized_at`, never `created_at`). The same
  user may also be a daily publisher.
- `inactive_writers_30d` and `inactive_writers_60d`: population with at least three
  finalized traces **as of that day**, and no writing in the preceding 30 or 60
  days. This is a population/stock, not the number newly entering inactivity.
  Reading does not disqualify writing inactivity. Historical points use the end
  of the UTC day; today's point stops at request time; future days are zero.
- `document_opened` and `album_opened`: counts of validated usage events. Like the
  existing `post_opened` event series, these are grouped by each user's local day.

The population is the same as existing user activity metrics: human platform users.
Writing means retained `USER_TRACE`/`WORKSPACE_TRACE` records with `finalized_at`;
drafts, complements, mentor output and WAL entries are not writing events here.
Archived writing retains its writing history, but is no longer treated as shared.

Sharing is evaluated **now**, with the same grants/mentions/blog/public-link
predicate as `/me/feed`. An otherwise published post with zero external access
is private. Grant changes, blocked access and link expiry may revise historical
sharing classifications; this is intentionally not visibility-history tracking.
A mention alone can grant access but does not create a publishing event.

`GET /admin/analytics/users/:id/series` also accepts `metric=document_opened` and
`metric=album_opened`. Existing date bounds and repeated `metric`
parameters still apply (default 30 days, maximum 366 inclusive days).

## Overlapping shares of total accounts over time

Use the same series route, selecting the three new share metrics and/or daily opens:

`GET /admin/analytics/platform_series?from=2026-10-01&to=2026-11-02&metric=publishers_share&metric=private_writers_share&metric=readers_share&metric=document_opened&metric=album_opened`

Share points add an explicit denominator and a percentage (0–100):

```json
{
  "metric": "publishers_share",
  "points": [
    {"date":"2026-11-02","count":12,"total_users":100,"percentage":12.0}
  ]
}
```

- `publishers_share`: accounts with at least three finalized traces currently shared.
- `private_writers_share`: accounts with at least three finalized traces and a
  strictly private majority, even if at least three other traces are shared.
- `readers_share`: accounts with any validated reading event in the preceding
  30 days, **regardless of the number of traces they have written**. Includes
  document/album opens as well as post/followed-journal opens.

These groups are independent. One account may qualify for all three, and their
percentages need not sum to 100. No churn-first classification is applied: an
inactive publisher/private writer still qualifies from their cumulative writing.

The denominator is **all human platform accounts created by that day**, not only
active accounts. Writing thresholds use only traces finalized by that day;
future writing/reading/account creation never leaks into earlier points. Shared
versus private is still classified using current access, so publishing or revoking
access later can revise historical shares. Unlike `daily_publishers` (a publication
activity metric), these shares describe cumulative writing classified by current
visibility, not historical grant/publication-state snapshots. Historical totals
use retained account/trace rows, just like the existing `users` series; deleted
accounts and deleted traces are not reconstructed.

Days are UTC; historical points use end of day and today's point stops at request
time. Days with no accounts (and future days) return zero counts and 0.0 percent.
For share charts, plot `percentage`; `count` is always an integer numerator. Old
count-only metrics retain exactly `{date,count}` with no extra fields. Daily
`document_opened` / `album_opened` series count opens, not distinct readers.

## Current user breakdown

`GET /admin/analytics/user_segments` (no date parameters; load independently of
the lightweight summary and daily charts).

```json
{
  "as_of": "2026-11-02T18:00:00Z",
  "total_users": 120,
  "segments": {
    "publishers": 30,
    "private_writers": 35,
    "readers": 25,
    "churners": 20,
    "other": 10
  }
}
```

Exactly one segment per user, applying this priority:

1. **Churners**: account older than 30 days, no writing or reading during the last
   30 days. Accounts never active qualify once old enough; brand-new accounts do not.
2. **Private writers**: at least three finalized traces, strictly more than half
   currently private (even if at least three other traces are shared).
3. **Publishers**: at least three distinct authored traces currently shared.
4. **Readers**: fewer than three finalized traces, reading within the last 30 days.
5. **Other**: new/intermediate users not matching the above.

Counts add up to `total_users`. Writing totals are lifetime retained trace counts;
the inactivity window is 30 elapsed days, not a calendar month. Segment churn is
different from the writing-only inactivity series.

Reading uses validated `FOLLOWED_JOURNAL_OPENED`, `POST_OPENED`, `DOCUMENT_OPENED`
and `ALBUM_OPENED` events. Existing active-reader 7/30-day analytics use this same
expanded set. Opens are an activity proxy, not proof that content was fully read.

## Client-reported open events

Send the existing `POST /usage_events` route after opening the actual screen,
not on prefetch or every build/render. A regular authenticated session is required.

```json
{"event_type":"document_opened","resource_id":"document-uuid"}
```

```json
{"event_type":"album_opened","resource_id":"album-uuid"}
```

`context_json` remains optional. The response is the existing `UsageEvent` object.
Missing resource ID is 400; nonexistent resource is 404; missing read access uses
the existing unauthorized response (401). Owners can report their own opens.
The server does not mark all contained traces as seen, modify access or record
reads automatically on collection/detail fetches. Clients must begin sending the
events; old document/album reading history cannot be reconstructed.

## Deployment

Apply `2026-11-02-180000_document_album_usage_events` before accepting new events
(normal server startup migrations do this). No new analytics tables are created.
The downgrade refuses to proceed if new event types already exist, rather than
silently deleting activity history.

Legacy timezone aliases `Asia/Calcutta` and `Asia/Saigon` normalize to `Asia/Kolkata`
and `Asia/Ho_Chi_Minh` for PostgreSQL. Bulk analytics validate remaining timezone
names against PostgreSQL's catalog and fall back to UTC for empty/unsupported
names so a single bad stored value cannot fail the platform-wide query.
