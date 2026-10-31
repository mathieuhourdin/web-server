# Production blog hostname

DNS: an A record for `blog.hupo.fr` pointing to the production VM's external IPv4
address. No extra DNS record is required for ordinary HTTPS validation. Do not
add an AAAA record unless the VM actually serves this hostname over IPv6.

The live nginx configuration is `/home/mathieu/nginx.conf`, bind-mounted into
`mathieu-nginx-1`. The repository's `nginx/nginx.conf` is a different, local-dev
configuration. Do not replace production with it.

The shared certificate named `hupo.fr` covers `hupo.fr`, `api.hupo.fr`,
`app.hupo.fr`, not `blog.hupo.fr`. A separate certificate named `blog.hupo.fr`
was provisioned on 2026-10-31 (initial expiry 2027-01-29). `/etc/letsencrypt` is
already mounted into nginx.

The VM already uses Certbot standalone renewal hooks in `ops/certbot/`. Staying
with that method requires briefly stopping nginx to let Certbot bind port 80.
Plan that interruption explicitly; do not run it during an unrelated deployment.
The post hook uses `up -d --no-deps nginx` to avoid recreating the backend.
After stopping nginx, the certificate issuance command on the VM is:

```sh
sudo certbot certonly --standalone --cert-name blog.hupo.fr -d blog.hupo.fr
```

Restart nginx even if issuance fails. Leave the live configuration unchanged
until the certificate exists. This document and snippet do not provision a
certificate or change production automatically.

Blog routing was installed on 2026-10-31. For a fresh installation, transfer
`install-nginx.sh` and `nginx.conf.example` into the same directory on the VM,
then run `sudo sh install-nginx.sh`. It creates a backup, preserves the bind-mount
inode, validates and reloads nginx, and restores the backup if installation fails.
It refuses to duplicate an existing blog server block.

Alternatively, back up `/home/mathieu/nginx.conf` and insert the two server blocks
from `nginx.conf.example` inside its existing `http {}` block. Keep every existing
server block. Then, on the VM:

```sh
docker exec mathieu-nginx-1 nginx -t
docker exec mathieu-nginx-1 nginx -s reload
```

Only reload after a successful test. No container recreation is required for
the nginx change. Configure `BLOG_BASE_URL=https://blog.hupo.fr` for the backend
and deploy/restart it using the normal workflow. The blog backend must be deployed
before these pages work; routing alone cannot add the new API routes.

Public routing is `https://blog.hupo.fr/<slug>` →
`http://web-server:3000/blog/<slug>`, preserving the rest of the path and query.
Management stays on `https://api.hupo.fr/blogs`, behind authentication.
