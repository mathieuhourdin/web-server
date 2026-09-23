# Long-term pipeline playground

This local-only experiment runner reuses the production trace extraction,
summary, context, element, and landmark pipeline. It never starts Axum, WAL
workers, or normal development PostgreSQL. Private datasets, generated raw
artifacts, model-call payloads, reports, and dumps are under `generated/` and
are gitignored.

Start the isolated PostgreSQL 17 service (container/volume are dedicated and
the port binds only to localhost):

```sh
cp playground/pipeline/.env.example playground/pipeline/.env
# edit its password
set -a; source playground/pipeline/.env; set +a
docker compose --env-file playground/pipeline/.env -f playground/pipeline/docker-compose.yml up -d
cargo run --bin pipeline_playground -- validate
cargo run --bin pipeline_playground -- init
```

Every lifecycle, checkpoint and branch command rejects a `DATABASE_URL` whose
database is not named `pipeline_*`. The runner sets `PIPELINE_PLAYGROUND_MODE=1`.
This retains and audits persisted local mentor feedback/messages, but suppresses
daily recap outbound-email scheduling and weekly push dispatch; the synthetic
user has `.invalid` email and email notifications disabled as a second guard.

## Dataset contract and commands

Download the authenticated all-traces export from
`GET /me/traces/export?format=json` (optional `from`/`to` dates). It includes
journal metadata and stable trace IDs, dates, content, versions, status and
types. Keep the export outside this worktree if preferred.

```sh
cargo run --bin pipeline_playground -- import --dataset /private/traces.json
cargo run --bin pipeline_playground -- run
# Or run autonomously until the queue is empty, checkpointing each analysis:
cargo run --bin pipeline_playground -- run-all
# Bound a trial while tuning prompts:
cargo run --bin pipeline_playground -- run-all --max 20
cargo run --bin pipeline_playground -- report
```

`import` applies embedded migrations, creates a synthetic valid human/service
mentor pair and journals, preserves journal and trace IDs plus metadata, then
creates/plans a normal lens queue. It rejects encrypted traces because the
server analysis pipeline cannot decrypt them. `run` uses the production
`run_lens_one` queue primitive, requires the new head to be `COMPLETED`, then
writes `generated/analyses/<id>/raw.json` and `manifest.json`. Raw artifacts
contain the persisted analysis/lens/inputs/elements/landmarks/summaries/mirrors/references,
local mentor feedback, and `llm_calls` (including model, prompt and
request/response fields when available).

`run` performs one queue step and exits successfully when there is no work.
`run-all` repeats that same atomic step until the queue is empty (or `--max N`
is reached), so every analysis still gets its own artifact and database dump.

Source assets are not exported by the all-traces JSON contract. The importer
therefore clears `content_image_asset_id`: it preserves all text and
analysis-relevant metadata, but cannot replay private image bytes or retain a
dangling asset foreign key.

With the default `PIPELINE_PLAYGROUND_CHECKPOINT=every`, each successful run
also makes a compressed custom-format pg_dump and records its path, size and
SHA-256 in the manifest. Full snapshots after every analysis grow O(n²); dumps
are never deleted automatically. Use `checkpoint [label]` for another manual
dump. `PIPELINE_PLAYGROUND_CHECKPOINT=never` is possible only for disposable
experiments and removes the exact checkpoint guarantee.

## Branches

One restored PostgreSQL database is one branch. A checkpoint represents state
after that manifest's completed analysis. Create a non-destructive branch:

```sh
cargo run --bin pipeline_playground -- branch \
  --from playground/pipeline/generated/dumps/<checkpoint>.dump \
  --database pipeline_prompt_variant
DATABASE_URL=postgres://pipeline:...@127.0.0.1:54321/pipeline_prompt_variant \
  cargo run --bin pipeline_playground -- run --lens <lens-id>
```

The command creates the target only if absent, then uses `pg_restore`; it never
overwrites an existing branch. `pg_dump`, `pg_restore`, and `psql` must be on
PATH. No private dataset is committed or copied by the tool.
