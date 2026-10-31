#!/bin/sh
# Run on the production VM, beside nginx.conf.example, after issuing its cert.
set -eu

config=/home/mathieu/nginx.conf
snippet="$(dirname "$0")/nginx.conf.example"
container=mathieu-nginx-1

test -r "$snippet"
test -f /etc/letsencrypt/live/blog.hupo.fr/fullchain.pem
if grep -q 'server_name blog.hupo.fr' "$config"; then
    echo 'Blog server already configured; inspect before changing it again.' >&2
    exit 1
fi
test "$(grep -c '^http {' "$config")" = 1
docker exec "$container" nginx -t

backup=$(mktemp /home/mathieu/nginx.conf.before-blog.XXXXXX)
staged=$(mktemp /tmp/hupo-blog-nginx.XXXXXX)
cp -p "$config" "$backup"
awk -v snippet="$snippet" '
    /^http \{$/ {
        print
        while ((getline line < snippet) > 0) print line
        close(snippet)
        next
    }
    { print }
' "$config" > "$staged"

success=false
restore() {
    status=$?
    if [ "$success" != true ]; then
        cp -p "$backup" "$config"
        docker exec "$container" nginx -t && docker exec "$container" nginx -s reload
        echo "Restored nginx configuration from $backup" >&2
    fi
    exit "$status"
}
trap restore EXIT
# Preserve the destination inode: nginx has this file bind-mounted.
cp "$staged" "$config"
docker exec "$container" nginx -t
docker exec "$container" nginx -s reload
success=true
echo "Blog routing installed. Previous configuration: $backup"
