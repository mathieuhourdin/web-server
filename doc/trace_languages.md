# Trace language detection

Trace responses include a read-only nullable `language_code`, for example `"fr"`
or `"en"`. This also applies to journal/search collections, public journal links,
feed cards, album trace items, continuation previews and trace-backed posts.
Non-trace feed items return `null`.

Detection runs locally using whatlang when plaintext traces are finalized,
including timeout finalization, directly finalized imports and complements.
It does not depend on AI consent, use an LLM, or incur provider costs.
Changes to finalized writing recompute it; metadata-only edits reuse the value.
Drafts and encrypted traces have no detected language.

The detector considers up to 16,384 characters of content (falling back to the
title if content is empty), strips common URL/HTML/fenced-code noise, requires
at least 20 alphabetic characters and a reliable detector result. Short,
ambiguous or unsupported writing returns `null`. This is a single dominant
language, not a multilingual span annotation or the author's preferred locale.
Codes use ISO 639-1 (Mandarin normalized to `zh`, Persian to `fa`).

Apply the migration before deploying. Existing records initially return `null`.
To populate existing finalized, unencrypted writing, explicitly run against the
intended database:

```sh
cargo run --release --bin backfill_trace_languages -- --batch-size 200
```

`--after UUID` resumes after a printed checkpoint. Each batch commits separately;
unknown results are skipped for the rest of that run. Re-running is safe. The
command prints counts and checkpoint IDs only, never writing content. It does
not change edit versions or timestamps and does not enqueue analyses.
