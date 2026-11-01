# Lean request timing

No new service, dependency, environment variable or database migration. Nothing
here changes authentication, connection-pool limits, retry behavior or timeouts.
No runtime heartbeat or mobile changes are required for this first step.

## What is recorded

- `X-Request-Id` remains optional. Nginx preserves a valid UUID (canonical or
  32-hex form), otherwise generates a 32-hex ID. It forwards it to Rust. Requests
  sent directly to Rust also work: Rust generates an ID when needed. The ID is
  returned on application responses. It is untrusted diagnostic metadata, not
  authentication or an idempotency key; clients should use a fresh ID per attempt.
- Rust starts the request span **before** session resolution and the automation
  gate. `http_request_started` and `http_request_completed` share that context.
  Completion `latency_ms` includes authentication, authorization, handler work
  and response construction; it does **not** include body delivery or time spent
  waiting to be scheduled before the middleware runs. `auth_ms` measures session
  resolution, including secret verification, database reads and last-seen writes,
  but not the separate automation gate.
- Existing error logs retain their request/user/session context. Authentication
  timing also appears in the request span. Session metadata is passed internally
  through response extensions, never serialized into the response body.
- `http_request_aborted` means the request future was dropped or panicked before
  producing a response. It does not prove a client disconnect, and is not a
  body-delivery completion event. A blocking operation can delay this log too.
- The central r2d2 hooks log `db_pool_checkout_slow` for acquisition >=100 ms,
  `db_pool_checkout_timeout` for every acquisition timeout, and
  `db_pool_connection_held_long` for connections held >=5 seconds. Acquisition
  includes connection validation, not just queue waiting. `held_ms` is connection
  lifetime outside the pool, **not** SQL execution time. These events inherit the
  current request span when available; background work can have no request ID.
  Callbacks deliberately do not inspect pool state or acquire another connection.
- Nginx records status, body bytes, total request duration, upstream connect time,
  time to upstream headers and upstream response duration. Values are seconds,
  may be `-` or lists for upstream retries, and overlap: do not add them together.
  A 499 means the client closed the request before completion, not why it closed.

Query strings, authorization headers, cookies, bodies, SQL parameters and trace
content are not added to these diagnostic logs. Existing unrelated application
logs are unchanged. Request IDs and resource paths are not metric labels.

## Nginx deployment

The repository's `nginx/nginx.conf` is the local-development configuration.
**Do not copy it over production**: production has separate API, web and blog
hosts, TLS certificates and routes in `/home/mathieu/nginx.conf`, bind-mounted
into `mathieu-nginx-1`.

When deploying this feature:

1. Back up the live config. Copy the marked `BEGIN/END HUPO REQUEST OBSERVABILITY`
   block from `nginx/nginx.conf` **inside its existing `http {}` block**.
2. In every location proxying to Rust (API and blog), add:

   ```nginx
   proxy_set_header X-Request-Id $hupo_request_id;
   ```

   Put it alongside the existing `proxy_set_header` directives: nginx does not
   inherit a parent list when the child declares its own proxy headers.
3. Remove/replace existing access-log directives at the relevant server/location
   levels if they override the new `http`-level `access_log`. Keep unrelated
   routes, proxy headers, certificates and logging-agent configuration intact.
4. Preserve the live file's inode when editing (it is bind-mounted), then run:

   ```sh
   docker exec mathieu-nginx-1 nginx -t
   docker exec mathieu-nginx-1 nginx -s reload
   ```

   If validation fails, restore the backup before reloading. This change does
   not require restarting Rust or rebuilding nginx.
5. Ensure the existing Cloud Logging pipeline collects nginx stdout **and parses
   the new JSON payload** instead of assuming the old combined access-log format.
   Check that `request_id` and timing fields are queryable, not buried as a JSON
   string in `textPayload`. The backend deployment alone does not install the
   production nginx/agent configuration.

## Finding a slow request

A client can optionally send a fresh UUID in `X-Request-Id` and record it together
with elapsed time and the endpoint, including on receive timeout. Older clients
do not need an update; nginx and Rust still share a generated ID.

Search Cloud Logging for the ID in both events and their inherited spans:

```text
jsonPayload.request_id="REQUEST_ID" OR jsonPayload.spans.request_id="REQUEST_ID"
```

Compare the nginx entry to Rust start/completion and any pool warnings:

- Slow acquisition: investigate pool exhaustion or connection validation.
- High `auth_ms`, fast acquisition: investigate session verification/queries.
- Nginx waits for headers but Rust reports little elapsed time: look at scheduling
  delays before Rust's timer starts, proxy delivery and the matched request ID.
- Fast upstream response but slow nginx total/client time: investigate response
  delivery or the client/network. These are clues, not automatic diagnoses.

All normal request logs are retained for now; only slow/failed pool events emit
warnings. No exporter, dashboard, paid monitoring product or sampling policy is
introduced by this change.
