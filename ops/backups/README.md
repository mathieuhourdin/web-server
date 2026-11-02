# Scheduled production database backups

This is the VM-side job. Keep `../backups/backup-prod-db.sh` for manual downloads
from your laptop; do not run that SSH/SCP script from cron on the VM.
No server rebuild, Docker Compose changes or backend restart is needed.
See [production.md](production.md) for the verified production installation and
first successful run; the heartbeat itself remains outside Git.

## Install on the production VM

Copy these files to the VM first, then run from their directory:

```bash
sudo bash install.sh
sudoedit /etc/hupo-db-backup.env
```

Set `BACKUP_HEARTBEAT_URL` to the secret Better Stack URL. Do not overwrite an
existing configuration on subsequent script updates. No password or service-account
key is required when PostgreSQL permits the existing local container connection; uploads
use the attached VM service account's short-lived token, not a personal gcloud login.
The VM-side backup does not require gcloud; it uploads with curl over HTTPS.

Before scheduling, run one real backup using the same identity and command:

```bash
sudo timeout --signal=TERM --kill-after=60s 20m /usr/local/sbin/hupo-db-backup
```

Only after this succeeds, enable cron:

```bash
sudo bash install.sh --enable-cron
```

`/etc/cron.d/hupo-db-backup` is where the schedule lives. Unlike a user crontab,
each entry includes `root` as the execution user. It runs at 03:00 and 15:00 in
the VM's system timezone, verified as Etc/UTC. Cron picks up the installed file
automatically; no reboot is needed. If the VM timezone changes, review the schedule.
The installer preserves existing config and refuses to enable cron without a
successful backup newer than the installed script and config. Installation does
not disable or rewrite any existing application schedules. Re-running `install.sh`
updates the backup script only, preserving an already enabled backup schedule.
See `../cron/README.md` for sanitized copies of existing production jobs.

Configure the Better Stack heartbeat to expect success every **12 hours**, with
**30 minutes' grace**. The first real successful run activates heartbeat monitoring;
do not send a dummy success ping. Explicit errors call `/fail`, while a missing
heartbeat catches failures where the script never starts or the VM is unavailable.

## Upload permissions and verification

The script uploads to unique exact object names under:

```
gs://hupo-prod-db-backups/postgres/YYYY/MM/DD/postgres_TIMESTAMP_UUID.dump
```

The script uses a single HTTPS PUT to the GCS XML API, not `gcloud storage cp`.
The installed CLI was tested and attempted an object GET despite create-only
upload flags. Direct PUT avoids this and requires no object list/get/delete access.
The `x-goog-if-generation-match: 0` header prevents overwriting a live object;
`Content-MD5` asks GCS to validate the exact dump bytes before accepting the upload.
The bearer token is passed through curl's stdin configuration, never its command
arguments. IAM permissions and the VM's OAuth access scopes must both permit the
upload. No additional Storage Admin/Viewer rights are needed.

References: [GCS object upload](https://docs.cloud.google.com/storage/docs/xml-api/put-object-upload)
and [generation preconditions](https://docs.cloud.google.com/storage/docs/xml-api/reference-headers#xgoogifgenerationmatch).

The archive must be nonempty and `pg_restore --list` must succeed. This checks its
structure, not full restorability. Schedule separate periodic restore tests in an
isolated database without inspecting or logging personal content.

Before dumping, the script reads only `pg_database_size(current_database())` to
check local authentication and disk requirements. It requires enough free space
for twice that size (at least 1 GiB), plus a 2 GiB safety reserve configured with
`BACKUP_MIN_FREE_BYTES`. A Linux Bash file-size limit bounds the dump to that
budget. If space is insufficient or the limit is hit, it fails and alerts instead
of trying to fill the disk. Other concurrent workloads can still consume the
reserve; continue monitoring VM disk space. No tables, authentication rules,
database volumes or existing backups are modified.

## Operations and privacy

```bash
sudo cat /etc/cron.d/hupo-db-backup
sudo journalctl -t hupo-db-backup --since today
```

Logs contain only stage, exit code, object name, size and duration. Dump contents,
credentials and the heartbeat URL are never logged or sent to Better Stack.
On a command failure, up to 64 KiB of stderr is kept in the root-only
`/var/lib/hupo-db-backup/last-error.log`; treat it as potentially sensitive and
redact before sharing. Temporary dump/credential files are cleaned up on exit.
The lock prevents overlapping runs; cron's 20-minute timeout bounds the job.
If a timeout kills the process before it can report failure, the missing heartbeat
still alerts. If the success heartbeat fails after upload, the job exits nonzero
with `stage=heartbeat` and `uploaded=true`; the uploaded backup remains intact.

This backs up the `postgres` database, not GCS assets, PostgreSQL cluster-wide roles
or VM configuration. It does not provide point-in-time recovery. Set bucket privacy,
retention/lifecycle and restore-reader access separately. The writer should not
have permission to delete previous backups.
