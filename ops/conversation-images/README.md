# Temporary conversation images

Use NEW private buckets only; never add lifecycle deletion to existing assets or
database-backup buckets. The runtime service account needs object create/get/delete
access scoped to the corresponding bucket. The existing ADC URL signer must also
have read access; if it is a different service account, grant it bucket-scoped
`roles/storage.objectViewer`. Reuse the existing IAM signBlob setup; do not create
JSON service-account keys or replace the VM's attached account.

Example production setup (creates cloud resources):

```bash
bash ops/conversation-images/setup-bucket.sh \
  ppdc-infra hupo-conversation-images-prod europe-west9 \
  web-server-prod@ppdc-infra.iam.gserviceaccount.com
```

For development use `hupo-conversation-images-dev` and the dev runtime account.
If local ADC impersonates another account, give that account access to the dev
bucket, not production. Set `GCS_CONVERSATION_IMAGES_BUCKET` in the actual API
container environment and recreate the API service to apply it. Merely defining
it in the host's shell does not pass it into Docker Compose. No new cron job.

The script refuses an existing bucket, including a partially configured bucket
from an interrupted run. Inspect and complete that bucket explicitly rather than
removing/recreating it or changing an unrelated bucket. CORS is for browser reads
only; uploads go through the API. Adapt allowed origins if the deployed frontend
uses another domain. CORS is not authorization and does not make objects public.

Check metadata only:

```bash
gcloud storage buckets describe gs://hupo-conversation-images-prod --project=ppdc-infra
gcloud storage buckets get-iam-policy gs://hupo-conversation-images-prod --project=ppdc-infra
```

Expected: Standard regional storage, uniform access, public-access prevention,
one Delete/age=45 rule, no object versioning, zero soft-delete retention, no bucket
retention lock. GCS removes objects asynchronously. API expiry is independently
checked against the upload reservation's 45-day deadline. Signed read URLs last
at most five minutes and are capped at that deadline. GCS bytes aren't part of DB
backups, so restoring PostgreSQL does not recover expired images.

The API stores no public URL, sends no image URL in pushes, validates actual image
bytes under limits, and leaves compression/orientation/EXIF stripping to the
client. Account purge covers both permanent assets and these temporary objects;
already-deleted GCS objects count as successfully removed. No general asset-model
changes or cross-resource image reuse are introduced.
