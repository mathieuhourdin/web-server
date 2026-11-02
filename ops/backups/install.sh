#!/usr/bin/env bash
# Run on the target Linux VM. No application/container restart is performed.
set -euo pipefail
umask 077

[[ $EUID == 0 ]] || { echo 'Run with sudo.' >&2; exit 1; }
source_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
mode=${1:-install}
case "$mode" in
    install)
        [[ $# -le 1 ]] || exit 2
        for dependency in bash docker curl python3 openssl flock timeout logger; do
            command -v "$dependency" >/dev/null
        done
        bash -n "$source_dir/backup-to-gcs.sh"
        install -o root -g root -m 0750 "$source_dir/backup-to-gcs.sh" /usr/local/sbin/hupo-db-backup
        install -d -o root -g root -m 0700 /var/lib/hupo-db-backup
        if [[ ! -e /etc/hupo-db-backup.env ]]; then
            install -o root -g root -m 0600 "$source_dir/backup.env.example" /etc/hupo-db-backup.env
            echo 'Created /etc/hupo-db-backup.env; set its secret heartbeat URL before running.'
        fi
        # Never overwrite existing per-VM credentials or existing cron jobs.
        echo 'Installed script. Existing config and schedules are preserved.'
        echo 'Run a successful backup, then run: sudo bash install.sh --enable-cron'
        ;;
    --enable-cron)
        [[ $# == 1 ]] || exit 2
        [[ -x /usr/local/sbin/hupo-db-backup && -r /etc/hupo-db-backup.env ]] || {
            echo 'Install and configure the backup first.' >&2; exit 1;
        }
        [[ -f /var/lib/hupo-db-backup/last-success &&
           /var/lib/hupo-db-backup/last-success -nt /usr/local/sbin/hupo-db-backup &&
           /var/lib/hupo-db-backup/last-success -nt /etc/hupo-db-backup.env ]] || {
            echo 'Run a successful backup with this script and config before enabling cron.' >&2; exit 1;
        }
        timezone=$(timedatectl show -p Timezone --value)
        [[ "$timezone" == Etc/UTC || "$timezone" == UTC ]] || {
            echo 'Cron template expects a UTC VM. Review the schedule for this VM first.' >&2; exit 1;
        }
        systemctl is-active --quiet cron
        if [[ -e /etc/cron.d/hupo-db-backup ]] && ! cmp -s "$source_dir/hupo-db-backup.cron" /etc/cron.d/hupo-db-backup; then
            echo 'Existing backup schedule differs; review it manually rather than overwriting.' >&2; exit 1
        fi
        install -o root -g root -m 0644 "$source_dir/hupo-db-backup.cron" /etc/cron.d/hupo-db-backup
        echo 'Enabled backups at 03:00 and 15:00 UTC in /etc/cron.d/hupo-db-backup.'
        ;;
    *) echo 'Usage: sudo bash install.sh [--enable-cron]' >&2; exit 2 ;;
esac
