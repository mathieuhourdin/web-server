#!/usr/bin/env bash
# VM-side backup. Run with a 20-minute timeout (see hupo-db-backup.cron).
# Never enable shell tracing: config contains a secret heartbeat URL.
set +x
set -Eeuo pipefail
umask 077

stage=config
workdir=''
stderr_file=''
uploaded=false
started_at=$SECONDS

log() { printf '%s event=%s stage=%s %s\n' "$(date -u +%FT%TZ)" "$1" "$stage" "${2:-}"; }

heartbeat() {
    # Pass the secret URL on stdin, not in curl's process arguments or logs.
    printf 'url = "%s"\n' "${BACKUP_HEARTBEAT_URL%/}${1:-}" |
        curl --config - --silent --fail --output /dev/null \
            --connect-timeout 5 --max-time 10 --retry 2 --retry-delay 1
}

cleanup() {
    if [[ -n "$workdir" && "$workdir" == "${BACKUP_TMP_DIR:-/var/tmp}"/hupo-db-backup.* && -d "$workdir" ]]; then
        # Only the private directory created by this run, never a configurable root.
        rm -rf -- "$workdir"
        workdir=''
    fi
}

on_exit() {
    local status=$?
    trap - EXIT TERM INT
    set +e
    if (( status != 0 )); then
        log failed "exit_code=$status uploaded=$uploaded"
        if [[ -n "$stderr_file" && -f "$stderr_file" && -d "${BACKUP_STATE_DIR:-}" ]]; then
            # Private, bounded diagnostic only; never send raw output to monitoring.
            tail -c 65536 "$stderr_file" > "$BACKUP_STATE_DIR/last-error.log"
            chmod 600 "$BACKUP_STATE_DIR/last-error.log"
        fi
        if [[ "${heartbeat_configured:-false}" == true ]]; then
            heartbeat /fail 2>/dev/null || log heartbeat_delivery_failed
        fi
    fi
    cleanup
    exit "$status"
}
trap on_exit EXIT
trap 'exit 124' TERM
trap 'exit 130' INT

config_file=${BACKUP_CONFIG_FILE:-/etc/hupo-db-backup.env}
[[ -r "$config_file" ]] || { log failed 'reason=missing_config'; exit 1; }
# shellcheck disable=SC1090
source "$config_file"
: "${BACKUP_HEARTBEAT_URL:?Set BACKUP_HEARTBEAT_URL in the private config}"
[[ "$BACKUP_HEARTBEAT_URL" =~ ^https://incidents\.betterstack\.com/api/v1/heartbeat/[A-Za-z0-9_-]+/?$ ]] || {
    log failed 'reason=invalid_heartbeat_url'; exit 1;
}
heartbeat_configured=true

BACKUP_BUCKET=${BACKUP_BUCKET:-hupo-prod-db-backups}
BACKUP_COMPOSE_FILE=${BACKUP_COMPOSE_FILE:-/home/mathieu/docker-compose.yml}
BACKUP_DB_SERVICE=${BACKUP_DB_SERVICE:-db}
BACKUP_DB_USER=${BACKUP_DB_USER:-postgres}
BACKUP_DB_NAME=${BACKUP_DB_NAME:-postgres}
BACKUP_STATE_DIR=${BACKUP_STATE_DIR:-/var/lib/hupo-db-backup}
BACKUP_LOCK_FILE=${BACKUP_LOCK_FILE:-/run/hupo-db-backup.lock}
BACKUP_TMP_DIR=${BACKUP_TMP_DIR:-/var/tmp}
BACKUP_MIN_FREE_BYTES=${BACKUP_MIN_FREE_BYTES:-2147483648}
[[ "$BACKUP_BUCKET" =~ ^[a-z0-9][a-z0-9._-]+$ ]] || { log failed 'reason=invalid_bucket'; exit 1; }
[[ "$BACKUP_MIN_FREE_BYTES" =~ ^[0-9]{1,12}$ ]] || { log failed 'reason=invalid_disk_reserve'; exit 1; }

stage=dependencies
for command in docker curl python3 openssl flock; do command -v "$command" >/dev/null; done
stage=lock
exec 9>"$BACKUP_LOCK_FILE"
flock --nonblock 9
mkdir -p -m 700 "$BACKUP_STATE_DIR"
chmod 700 "$BACKUP_STATE_DIR"
workdir=$(mktemp -d "$BACKUP_TMP_DIR/hupo-db-backup.XXXXXXXX")
stderr_file="$workdir/stderr.log"
dump_file="$workdir/database.dump"
compose=(docker compose -f "$BACKUP_COMPOSE_FILE" exec -T "$BACKUP_DB_SERVICE")

stage=preflight
# Metadata only. This also verifies the existing local authentication, without
# reading user records or changing PostgreSQL's authentication configuration.
database_bytes=$("${compose[@]}" psql --username="$BACKUP_DB_USER" --dbname="$BACKUP_DB_NAME" \
    --no-password --no-psqlrc --tuples-only --no-align \
    --command='SELECT pg_database_size(current_database())' 2> "$stderr_file")
[[ "$database_bytes" =~ ^[0-9]{1,12}$ ]] || { log failed 'reason=invalid_database_size'; exit 1; }
available_bytes=$(python3 -c 'import os,sys; s=os.statvfs(sys.argv[1]); print(s.f_bavail*s.f_frsize)' "$workdir")
# Allow twice the database size (at least 1 GiB), plus 2 GiB reserved for the VM.
# The file-size limit bounds this job's growth; it cannot reserve space against
# other concurrent workloads, so this is not a substitute for disk monitoring.
dump_limit_bytes=$((database_bytes * 2))
(( dump_limit_bytes >= 1073741824 )) || dump_limit_bytes=1073741824
dump_limit_kib=$(((dump_limit_bytes + 1023) / 1024))
required_bytes=$((dump_limit_kib * 1024 + BACKUP_MIN_FREE_BYTES))
(( available_bytes >= required_bytes )) || {
    log failed "reason=insufficient_disk available_bytes=$available_bytes required_bytes=$required_bytes"; exit 1;
}
log preflight_passed "database_bytes=$database_bytes available_bytes=$available_bytes dump_limit_bytes=$((dump_limit_kib * 1024))"

stage=credentials
# Use only the VM's attached service account, never a personal gcloud login.
curl --noproxy '*' --silent --show-error --fail --connect-timeout 3 --max-time 10 --retry 2 \
    -H 'Metadata-Flavor: Google' \
    'http://169.254.169.254/computeMetadata/v1/instance/service-accounts/default/token' \
    > "$workdir/metadata-token.json" 2> "$stderr_file"
python3 -c 'import json,sys; print(json.load(sys.stdin)["access_token"])' \
    < "$workdir/metadata-token.json" > "$workdir/access-token" 2> "$stderr_file"

stage=dump
log started
(
    # Linux Bash uses KiB here. Never let this dump consume all free VM space.
    ulimit -f "$dump_limit_kib"
    "${compose[@]}" pg_dump --username="$BACKUP_DB_USER" --dbname="$BACKUP_DB_NAME" \
        --no-password --format=custom --lock-wait-timeout=30s > "$dump_file" 2> "$stderr_file"
)
[[ -s "$dump_file" ]]

stage=archive_check
"${compose[@]}" pg_restore --list < "$dump_file" > /dev/null 2> "$stderr_file"
bytes=$(wc -c < "$dump_file" | tr -d '[:space:]')
md5=$(openssl dgst -md5 -binary "$dump_file" | openssl base64 -A)
run_id=$(python3 -c 'import uuid; print(uuid.uuid4())')
object="postgres/$(date -u +%Y/%m/%d)/postgres_$(date -u +%Y%m%dT%H%M%SZ)_${run_id}.dump"

stage=upload
# Direct XML API upload: gcloud cp can attempt object GET even with composite
# uploads disabled. This performs only a PUT, validates the checksum server-side,
# and cannot overwrite a live object. The bearer token is on stdin, not argv.
http_status=$(
    printf 'header = "Authorization: Bearer %s"\n' "$(< "$workdir/access-token")" |
        curl --config - --silent --show-error --fail --proto '=https' \
            --connect-timeout 10 --max-time 900 --request PUT --upload-file "$dump_file" \
            --header 'Content-Type: application/octet-stream' \
            --header "Content-MD5: $md5" --header 'x-goog-if-generation-match: 0' \
            --output /dev/null --write-out '%{http_code}' \
            "https://storage.googleapis.com/$BACKUP_BUCKET/$object" 2> "$stderr_file"
)
[[ "$http_status" == 200 ]] || { log failed "reason=unexpected_upload_status http_status=$http_status"; exit 1; }
uploaded=true

stage=cleanup
cleanup
stage=heartbeat
heartbeat 2>/dev/null
stage=complete
touch "$BACKUP_STATE_DIR/last-success"
log succeeded "object=gs://$BACKUP_BUCKET/$object bytes=$bytes duration_s=$((SECONDS-started_at))"
