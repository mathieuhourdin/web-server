#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 4 ]]; then
  echo "Usage: bash $0 PROJECT BUCKET REGION RUNTIME_SERVICE_ACCOUNT_EMAIL" >&2
  exit 2
fi
image_project=$1
image_bucket=$2
image_region=$3
image_service_account=$4
image_ops_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

if [[ ! "$image_bucket" =~ ^hupo-conversation-images-(dev|prod)$ ]] ||
   [[ "$image_service_account" != *@"${image_project}".iam.gserviceaccount.com ]]; then
  echo "Refusing unexpected bucket name or a service account outside the project." >&2
  exit 2
fi

# Never apply TTL/soft-delete changes to an existing bucket (especially assets).
# Creation itself fails if an inaccessible or globally taken bucket already exists.
if gcloud storage buckets describe "gs://${image_bucket}" --project="$image_project" >/dev/null 2>&1; then
  echo "Bucket already exists; refusing to change its lifecycle. Inspect it manually." >&2
  exit 1
fi

gcloud storage buckets create "gs://${image_bucket}" --project="$image_project" \
  --location="$image_region" --default-storage-class=STANDARD \
  --uniform-bucket-level-access --public-access-prevention --soft-delete-duration=0
gcloud storage buckets update "gs://${image_bucket}" --project="$image_project" \
  --lifecycle-file="${image_ops_dir}/lifecycle.json" --cors-file="${image_ops_dir}/cors.json"
gcloud storage buckets add-iam-policy-binding "gs://${image_bucket}" --project="$image_project" \
  --member="serviceAccount:${image_service_account}" --role=roles/storage.objectUser

echo "Configure the API: GCS_CONVERSATION_IMAGES_BUCKET=${image_bucket}"
