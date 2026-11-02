# Production installation record

Installed and verified on 2026-11-02; no backend deployment or container restart.

- VM: `matieregrise-server`, project `ppdc-infra`, zone `europe-west9-a`.
- Timezone: `Etc/UTC`; cron service active.
- Script: `/usr/local/sbin/hupo-db-backup`, root:root, mode 0750.
- Private config: `/etc/hupo-db-backup.env`, root:root, mode 0600.
- State/diagnostics: `/var/lib/hupo-db-backup`, root:root, mode 0700.
- Schedule: `/etc/cron.d/hupo-db-backup`, 03:00 and 15:00 UTC daily.
- Source DB: Compose `/home/mathieu/docker-compose.yml`, service `db`, database
  `postgres`, user `postgres`; existing local authentication verified, no password
  or authentication-rule changes required.
- Upload identity: attached VM account
  `web-server-prod@ppdc-infra.iam.gserviceaccount.com`; no IAM changes made.
- Destination: `gs://hupo-prod-db-backups`; public-access prevention enforced,
  uniform bucket-level access enabled, no public principals in the bucket policy.
- Secret heartbeat configured only on the VM; not included in this repository.

## First successful end-to-end run

- Completed: **2026-11-02 12:39:44 UTC**.
- Duration: **20 seconds**.
- Archive size: **98,493,326 bytes** (approximately 94 MiB).
- Object: `postgres/2026/11/02/postgres_20261102T123943Z_c80a7381-695b-4d42-a149-50f0ef3f8dc2.dump`.
- GCS MD5 (base64): `UuYPInLhua/IUcpcIRGkCg==`.
- Checks: nonempty custom-format dump, `pg_restore --list` (output discarded),
  GCS-validated Content-MD5, successful heartbeat, temporary dump/token cleanup.
- Object metadata independently verified without downloading or inspecting content.
- This is not a full restore drill. Assets and cluster-wide PostgreSQL roles are
  outside this backup's scope.

The initial `gcloud storage cp` test failed because the CLI attempted an object
GET. The final installed implementation uses direct HTTPS PUT and succeeded
without widening permissions. Do not switch back to the CLI without retesting
against the create-only identity.

The existing `mathieu` application crontab and `/etc/cron.d/certbot` were checked
by SHA-256 before and after installation and are unchanged. Their sanitized local
references and schedules are in `../cron/`. Containers remained running; no user
content was inspected. Runtime logs use `journalctl -t hupo-db-backup`.
