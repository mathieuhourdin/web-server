gcloud builds submit --config=cloudbuild.yaml
gcloud compute ssh --zone "europe-west9-a" "matieregrise-server" --project "ppdc-infra" \
  --command 'docker pull eu.gcr.io/ppdc-infra/ppdc-backend-api'
gcloud compute ssh --zone "europe-west9-a" "matieregrise-server" --project "ppdc-infra" \
  --command 'docker compose up -d'
gcloud compute ssh --zone "europe-west9-a" "matieregrise-server" --project "ppdc-infra" \
  --command 'docker images eu.gcr.io/ppdc-infra/ppdc-backend-api --format "{{.ID}}" | awk "!seen[\$0]++" | tail -n +6 | xargs -r docker rmi'
