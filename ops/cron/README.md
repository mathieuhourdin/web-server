# Production scheduled jobs

Keep application scripts and schedules in Git; keep actual credentials and
machine-specific configuration on the VM. Never export a raw production crontab
to Git: the existing application jobs contain an internal API authentication token.

## Inventory (matieregrise-server, 2026-11-02, Etc/UTC)

| Job | Schedule | Installed location | Local source/reference |
| --- | --- | --- | --- |
| Pending analyses / pipeline jobs | Every 20 minutes | `mathieu` user crontab | `production/mathieu.crontab.example` |
| Pending emails | Every 20 minutes | `mathieu` user crontab | `production/mathieu.crontab.example` |
| Shared-journal daily digests | Every 20 minutes | `mathieu` user crontab | `production/mathieu.crontab.example` |
| PostgreSQL backup to GCS | 03:00 and 15:00 UTC | `/etc/cron.d/hupo-db-backup` | `../backups/hupo-db-backup.cron` |
| Certificate renewal fallback | Twice daily, skipped under systemd | `/etc/cron.d/certbot` | `production/certbot.cron` |

The application snapshot is a **user crontab** (five schedule fields, no user
column). The backup and Certbot files are **system cron.d files** (include a user
column). Do not install either format in the other's location. No existing
application cron entries are changed by the backup installation.

`production/mathieu.crontab.example` contains `${INTERNAL_CRON_TOKEN}` instead of
the production value. It is documentation/a deployment template, not a directly
usable replacement: cron does not inherit application container variables. A future
Secret Manager deployment can supply this token through a private configuration
and a wrapper script, without committing the value or embedding it in a crontab.
Review and merge with an existing user crontab rather than replacing it blindly.

Certbot's file is package-owned. On systemd VMs inspect `certbot.timer`; merely
copying the fallback cron file does not enable renewal under systemd.
Standard OS maintenance entries (`/etc/crontab`, `e2scrub_all`, `sysstat`) remain
managed by their OS packages, not by this repository.

## Reuse on another VM

Use `../backups/install.sh` for the backup script and its schedule; see its README.
Adapt the Compose path/service, bucket, database and private heartbeat configuration
for that VM. Validate its timezone and run a real backup before enabling cron.
Use a separate heartbeat per VM/job so one healthy VM cannot mask another's failure.

Do not apply every file in this directory automatically: these are an explicit
inventory plus templates, not a blanket replacement of the VM's cron configuration.
