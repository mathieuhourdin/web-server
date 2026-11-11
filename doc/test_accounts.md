# Admin-managed test accounts

`is_test_account` is an admin-managed classification, independent of agent demo
accounts. Existing and newly registered users default to `false`.

## Mark or unmark an account

```http
PATCH /admin/users/:id/test_account
Content-Type: application/json

{"is_test_account":true}
```

Admin session required. Returns 200:

```json
{"id":"user-uuid","is_test_account":true}
```

Set `false` to unmark. Repeating the same value is safe. A missing boolean or
unknown request field returns 422, an unauthenticated request 401, a non-admin
403, and an unknown user 404. There is no body-less toggle.

`GET /admin/users` includes `is_test_account` on every item. Its optional
`is_test_account=true|false` query filter applies to both items and total; omitted
includes both kinds. Existing pagination and other filters still apply.

## Discovery and analytics

Marked users are excluded from `/users`, `/users/search`, `/users/suggested`,
mentor discovery and the internal fuzzy-name matcher, including collection
counts. They are also excluded from platform analytics (account and content
counts, activity/time series, segments/churn/shares, health/failure counts,
device breakdowns and writing-nudge metrics). Agent demo accounts are excluded
from the same analytics population, while their existing sandbox rules remain.

The `analytics_users` view applies this exclusion at read time; it stores no
aggregates. Marking/unmarking changes historical aggregate results too. Raw
records and usage events are retained. Admin account management and explicitly
requested per-user diagnostics, heatmaps, device lists and costs remain available.

This flag does **not** restrict login, AI usage, messages, notifications or
existing relationships/grants. It does not revoke access, hide already shared
content, disable public blogs or make an account private. Existing authorized
users can still open its profile/content directly. Normal user profile update
payloads cannot modify the flag, and public/self profile DTOs do not expose it.
Use the separate agent-demo system when isolated sandbox behavior is needed.

No frontend changes or production data mutations are included. Normal server
startup applies migration `2026-11-11-120000_user_test_accounts`.
