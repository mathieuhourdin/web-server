# Hupo API discovery

- `GET /api/schema`: OpenAPI 3.1 route catalog, generated from the actual router.
- `GET /api/docs`: this guide (Markdown).
- `GET /api/docs?topic=platform`: general API conventions and route contracts.
- Other topics: `automation`, `writing_threads`, `draft_mentor`, `publication_notifications`, `trace_translations`, `trace_languages`.

These documentation endpoints are public and contain no live user data, credentials
or configuration secrets. Reading documentation never grants permission to call a route.
Valid agent/delegated credentials may also fetch the docs without extra scopes.

## Current coverage

This is a lean discovery document, not a complete typed client-generation contract.
It includes every explicitly registered HTTP operation, path parameters, purpose,
authentication notes and the effective delegated-agent route scope. Common operations
have curated explanations; other summaries are derived from handler names.

Request/response payload schemas and most query parameters are not yet typed in
OpenAPI. They remain described in the linked Markdown contracts and Rust DTOs.
`x-coverage` and each operation's `x-payload-schema-status` make this explicit.
Do not interpret missing schemas as empty request bodies or unrestricted objects.
Implicit HEAD/OPTIONS, static files and the unknown-route fallback are not listed.

## Authentication and authorization

Normal API requests use the existing session bearer credential. Admin routes
additionally check the ADMIN role. Internal scheduler
routes require `x-internal-cron-token`. Public journal links use their sharing token.
Resource ownership, grants, blocks, AI consent and quotas remain enforced by handlers.

Agent workflow: administrator provisions a client → agent creates demo users → agent
requests a delegated demo-user token → agent calls explicitly allowed platform routes.
Read `/me/capabilities` and `/automation/contract` with the agent credential for its
effective permissions. A documented platform route is not automatically an agent tool.
An allowlisted route may still be denied by scopes, sandbox rules or resource checks.

## Maintaining this documentation

Router registrations use `api_route` and the documented method builders. They call
the same Axum handler/middleware and record route metadata; there is no runtime source
parser and no separately maintained inventory of paths or HTTP methods.
Keep the registration prefix in sync with its router's nesting prefix.

Add precise domain notes in `src/api_schema.rs::purpose` or the feature contract.
Future payload enrichment should use a standard Rust schema generator such as Utoipa
with schemas derived from the actual serde DTOs, rather than hand-copying every field.
Swagger UI / Redoc can then render the OpenAPI document without becoming its source
of truth. No database migration or new environment variables are needed for discovery.
