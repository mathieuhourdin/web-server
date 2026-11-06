# Device analytics and admin user-device inspection

Both endpoints require an authenticated user with the `admin` role. Non-admins
receive 403. These are read-only queries over existing device records: no new
tracking, migration, environment variable or scheduled job.

## Recently active device snapshot

`GET /admin/analytics/devices?active_within_days=30`

`active_within_days`: integer, default 30, inclusive range 1–366 (otherwise 400).
The time window is rolling UTC: `as_of - active_within_days` through `as_of`.

```json
{
  "as_of": "2026-11-06T12:00:00Z",
  "active_since": "2026-10-07T12:00:00Z",
  "active_within_days": 30,
  "total_users": 20,
  "active_users": 12,
  "active_devices": 15,
  "users_without_recent_device": 8,
  "multi_platform_users": 3,
  "platforms": [
    {"device_type":"ios","users_count":6,"devices_count":7,"users_percentage":50.0,"devices_percentage":46.666666666666664}
  ],
  "app_versions": [
    {"device_type":"ios","version":"1.4.2","users_count":6,"devices_count":7,"users_percentage":50.0,"devices_percentage":46.666666666666664}
  ],
  "operating_systems": [
    {"device_type":"ios","name":"iOS","version":"18.5","users_count":6,"devices_count":7,"users_percentage":50.0,"devices_percentage":46.666666666666664}
  ],
  "browsers": [
    {"device_type":"web","name":"Firefox","version":"144","users_count":2,"devices_count":2,"users_percentage":16.666666666666668,"devices_percentage":13.333333333333334}
  ]
}
```

Breakdown arrays in this example are abbreviated. Semantics:

- Eligible accounts are platform users with `principal_type = HUMAN`, excluding
  agent-managed demo accounts. This applies to `total_users` too.
- Active devices are eligible users' non-revoked devices whose `last_seen_at`
  falls within the window. Authentication refreshes this timestamp; it does not
  establish that the app is foregrounded or that someone actually read content.
- `active_users` counts distinct users with at least one active device.
- `users_without_recent_device = total_users - active_users`: includes inactive
  users and users with no registered device. It is **not** a measurement of how
  many active users failed to register a device.
- `multi_platform_users` counts users with multiple distinct platform values,
  not merely multiple devices on one platform.
- Platform values: `ios`, `android`, `web`, `desktop`, `unknown`.
- Missing/blank software metadata stays in a `null` bucket, not silently dropped.
  Browsers include web devices (even with missing browser metadata), plus other
  devices explicitly reporting a browser.
- All percentage denominators are `active_users` and `active_devices`, including
  browser breakdowns. Empty denominators yield 0. Users can occur in multiple
  platform/version buckets, so user percentages need not sum to 100%.
- Buckets sort by decreasing device count, then platform/name/version.
- This is a snapshot of **current device metadata**, not historical usage or an
  app-version adoption timeline. Device metadata can be changed/overwritten.

## One user's devices

`GET /admin/users/:user_id/devices?limit=20&offset=0`

Returns `{items, total, limit, offset}`. Default limit 20, maximum 100, offset
must be nonnegative. Missing user: 404. Existing user without devices: 200 with
an empty collection. This support endpoint includes all account kinds and
revoked devices, unlike aggregate analytics.

```json
{
  "items": [{
    "id": "device-uuid",
    "device_type": "ios",
    "name": "iPhone",
    "app_version": "1.4.2",
    "os_name": "iOS",
    "os_version": "18.5",
    "browser_name": null,
    "browser_version": null,
    "last_seen_at": "2026-11-06T11:00:00",
    "created_at": "2026-10-01T11:00:00",
    "revoked_at": null,
    "push": {
      "has_token": true,
      "provider": "fcm",
      "token_updated_at": "2026-11-05T11:00:00"
    }
  }],
  "total": 1,
  "limit": 20,
  "offset": 0
}
```

Order: `last_seen_at DESC NULLS LAST`, `created_at DESC`, `id DESC`.
Device timestamps without offsets follow the existing UTC timestamp convention.
`push.has_token` means a nonempty token is stored, **not** that delivery works or
that the device remains eligible for pushes. Check `revoked_at` and `provider`
as well. Provider is `fcm`, `apns`, `web_push`, `expo`, `unknown`, or null.
No raw push token or installation identifier is selected or returned.

## Admin UI guidance

Load device analytics independently/lazily, not as a prerequisite for the main
dashboard. Load a user's paginated device list only when their device section is
opened. Label token presence as “Token enregistré”, not “Notifications actives”.
Mark revoked devices clearly. Client-reported versions/names may be missing and
must not be treated as authoritative security attestations.
