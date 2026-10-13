# Restricted demo automation

This MVP manages demo users only. Mentor editing is not enabled. Demo ownership is
immutable and stored in `automation_demo_users`, not a mutable profile tag. Accounts
have generated `demo-<uuid>@automation.invalid` addresses and no usable normal login.
They may follow/share/message only within the same agent's sandbox. Existing service
mentors and mentor-role accounts are the messaging exception. Normal user authorization remains in effect.

## Admin provisioning (normal administrator session)

- `GET /admin/automation/clients?limit=20&offset=0`: paginated clients.
- `POST /admin/automation/clients`: `{ "name": "Demo agent", "scopes": ["demo_users:create", "demo_users:read", "demo_users:update", "demo_users:delete", "demo_users:delegate", "content:read", "content:write", "social:read", "social:write"], "max_demo_users": 100, "ai_daily_limit": 100 }`.
- `POST /admin/automation/clients/:id/token`: rotate the key and revoke existing delegated sessions.
- `DELETE /admin/automation/clients/:id`: permanently revoke the client. All its delegated credentials immediately stop working; accounts are retained until explicitly purged by an admin.

Creation/rotation return `{ "client": {...}, "token": "Bearer agent.<id>.<secret>" }`.
The secret is returned only once and stored hashed. Keep it out of source control.

## Agent management (agent token in Authorization)

- `GET /me/capabilities`: effective scopes, limits and sandbox rules.
- `GET /automation/contract`: machine-readable allowed delegated routes and Markdown
  contracts. This is not a complete generated OpenAPI document; unsupported routes
  stay unavailable to delegated credentials.
- `POST /automation/demo_users`: `{ "users": [{ "first_name": "Alice", "last_name": "Demo", "handle": "alice-demo", "biography": "Optional", "timezone": "Europe/Paris", "ai_features_enabled": false }] }`.
  Atomic batch of 1–20 users; returns an array of demo views.
- `GET /automation/demo_users?limit=20&offset=0`: paginated own demo views.
- `GET /automation/demo_users/:id`: own demo view.
- `PUT /automation/demo_users/:id`: replace editable profile fields using the same
  per-user body as creation; generated email, password, roles and ownership cannot
  be edited. Missing optional biography clears it; missing timezone uses Europe/Paris.
- `DELETE /automation/demo_users/:id`: permanent purge including assets; returns 204.
- `POST /automation/demo_users/:id/delegations`: `{ "scopes": ["content:read", "content:write", "social:read", "social:write"] }`.
  Returns `{ "user_id": "...", "api_client_id": "...", "token": "Bearer <session>.<secret>", "expires_at": "...", "scopes": [...] }`.

Demo views are `{ "account_kind": "demo", "created_by_api_client_id": "...", "user": {...} }`.
Use the delegated token, not the agent key, to call existing platform routes as that
user. Normal resource ownership/access checks still apply. The explicit allowed-route
list is returned by the contract endpoint; new platform routes are denied by default.
Delegation scopes must be a subset of the client's scopes, and expire after one hour.
Delegated tokens cannot create additional sessions, devices, login methods or admin roles.

## AI, visibility and notifications

AI is disabled by default. Enabling it and issuing AI-capable delegation requires the
`ai:use` scope. Existing per-user limits also apply.
AI-enabled demo users also require `ai:use` on delegated content writes, because those
writes can enqueue automatic analyses or WAL compilations.
Guarded AI requests reserve from one atomic rolling 24-hour agent request quota, including failed requests. This is
not a monetary spending cap or a bound on the number of underlying LLM calls.

Demo users are excluded from other users' discovery; same-agent demos can discover
each other. Grants cannot target another sandbox or the
whole platform. Public journal links are disabled. Push delivery is suppressed, and
demo outbound emails are recorded as SUPPRESSED, never dispatched. Demo readers cannot
read real users' platform-wide publications or leave seen-state footprints on them. Automation actions
are audited without request bodies or credentials.

Typical workflow: provision client → create users → delegate → create journals/traces
and sandbox follow relationships through normal routes → publish/share → purge demos.
No demo accounts, credentials or quotas are seeded by the migration.
