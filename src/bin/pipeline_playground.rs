//! Local, one-analysis-at-a-time replay runner. It never starts the web or WAL workers.
use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{Bool, Nullable, Text, Timestamp, Timestamptz, Uuid as SqlUuid};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;
use web_server::entities_v2::error::PpdcError;
use web_server::{
    db,
    entities_v2::{
        analysis_summary::AnalysisSummary,
        journal::Journal,
        lens::{Lens, LensProcessingState, NewLens},
        llm_call::LlmCall,
        message::Message,
        reference::Reference,
        trace::{Trace, TraceStatus},
        trace_mirror::TraceMirror,
        user::{
            EmailNotificationMode, HomeFocusView, JournalTheme, NewUser, User, UserPrincipalType,
            WeekAnalysisWeekday,
        },
    },
    work_analyzer::run_lens_one,
};

const ROOT: &str = "playground/pipeline/generated";
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[derive(Deserialize)]
struct ExportedTrace {
    trace: Trace,
    #[allow(dead_code)]
    journal_title: String,
}
#[derive(Deserialize)]
struct Export {
    schema_version: Option<u32>,
    journals: Vec<Journal>,
    traces: Vec<ExportedTrace>,
}
#[derive(Serialize)]
struct Manifest {
    analysis_id: Uuid,
    analysis_type: String,
    status: String,
    lens_id: Uuid,
    parent_analysis_id: Option<Uuid>,
    lens_head: Option<Uuid>,
    input_trace_ids: Vec<Uuid>,
    input_trace_mirror_ids: Vec<Uuid>,
    started_at: NaiveDateTime,
    completed_at: NaiveDateTime,
    elapsed_ms: u128,
    artifact: String,
    dump_path: Option<String>,
    dump_bytes: Option<u64>,
    dump_sha256: Option<String>,
    checkpoint_policy: String,
}

fn usage() -> ! {
    eprintln!("pipeline_playground <validate|init|import --dataset FILE|run [--lens UUID]|run-all [--lens UUID] [--max N]|checkpoint [label]|branch --from DUMP --database pipeline_name|report>");
    std::process::exit(2)
}
fn root() -> PathBuf {
    env::var("PIPELINE_PLAYGROUND_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(ROOT))
}
fn db_url() -> Result<String> {
    env::var("DATABASE_URL").context("DATABASE_URL is required")
}

fn validate_db_name(name: &str) -> Result<()> {
    if !name.starts_with("pipeline_")
        || name.len() == "pipeline_".len()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        bail!("refusing operation: database name must match pipeline_[A-Za-z0-9_]+")
    }
    Ok(())
}

fn safe_db(url: &str) -> Result<String> {
    let parsed = Url::parse(url)?;
    let name = parsed
        .path()
        .strip_prefix('/')
        .context("DATABASE_URL must contain a database name")?
        .to_owned();
    validate_db_name(&name)?;
    Ok(name)
}

fn validate_checkpoint_label(label: &str) -> Result<()> {
    if label.is_empty()
        || label.starts_with('.')
        || label.contains("..")
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        bail!("checkpoint label may contain only letters, numbers, _, - and .")
    }
    Ok(())
}
fn policy() -> Result<String> {
    let value = env::var("PIPELINE_PLAYGROUND_CHECKPOINT").unwrap_or_else(|_| "every".into());
    if matches!(value.as_str(), "every" | "never") {
        Ok(value)
    } else {
        bail!("PIPELINE_PLAYGROUND_CHECKPOINT must be every or never")
    }
}
fn init() -> Result<db::DbPool> {
    let url = db_url()?;
    safe_db(&url)?;
    let pool = db::create_pool();
    db::init_global_pool(pool.clone());
    pool.get()?
        .run_pending_migrations(MIGRATIONS)
        .map_err(|err| anyhow!(err.to_string()))?;
    Ok(pool)
}

fn pp<T>(result: std::result::Result<T, PpdcError>) -> Result<T> {
    result.map_err(|err| anyhow!(err.message))
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::create_dir_all(path.parent().context("artifact path lacks parent")?)?;
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
fn stamp() -> String {
    Utc::now().format("%Y%m%dT%H%M%S%.3fZ").to_string()
}
fn digest(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
fn dump(path: &Path) -> Result<(u64, String)> {
    let url = db_url()?;
    safe_db(&url)?;
    fs::create_dir_all(path.parent().context("dump path lacks parent")?)?;
    let status = Command::new("pg_dump")
        .args(["--format=custom", "--compress=9", "--file"])
        .arg(path)
        .arg(url)
        .status()
        .context("run pg_dump")?;
    if !status.success() {
        bail!("pg_dump failed: {status}")
    }
    Ok((fs::metadata(path)?.len(), digest(path)?))
}

fn synthetic_user(pool: &db::DbPool) -> Result<User> {
    let suffix = Uuid::new_v4().simple().to_string();
    let base = |email: String,
                first_name: String,
                last_name: String,
                handle: String,
                principal_type,
                mentor_id| NewUser {
        email,
        principal_type: Some(principal_type),
        mentor_id,
        first_name,
        last_name,
        handle,
        // These users cannot authenticate externally, but the schema still
        // requires a password hash.
        password: Some(Uuid::new_v4().to_string()),
        profile_picture_url: None,
        profile_picture_asset_id: None,
        is_platform_user: Some(principal_type == UserPrincipalType::Human),
        biography: None,
        pseudonym: None,
        pseudonymized: Some(false),
        high_level_projects_definition: None,
        journal_theme: Some(JournalTheme::White),
        current_lens_id: None,
        week_analysis_weekday: Some(WeekAnalysisWeekday::Monday),
        timezone: Some("UTC".into()),
        context_anchor_at: None,
        welcome_message: None,
        home_focus_view: Some(HomeFocusView::Projects),
        shared_journal_activity_email_mode: Some(EmailNotificationMode::Off),
        received_message_email_mode: Some(EmailNotificationMode::Off),
        mentor_feedback_email_enabled: Some(false),
        ai_features_enabled: Some(true),
        ai_features_enabled_by_admin: Some(true),
        onboarding_version: Some(0),
        external_captures_default_journal_id: None,
        mentor_specific_prompt: if principal_type == UserPrincipalType::Service {
            Some(String::new())
        } else {
            None
        },
    };
    let mut mentor_payload = base(
        format!("pipeline-mentor-{suffix}@invalid.example"),
        "Pipeline".into(),
        "Mentor".into(),
        format!("@pipeline-mentor-{suffix}"),
        UserPrincipalType::Service,
        None,
    );
    pp(mentor_payload.hash_password())?;
    let mentor = pp(mentor_payload.create(pool))?;
    let mut user_payload = base(
        format!("pipeline-user-{suffix}@invalid.example"),
        "Pipeline".into(),
        "Replay".into(),
        format!("@pipeline-replay-{suffix}"),
        UserPrincipalType::Human,
        Some(mentor.id),
    );
    pp(user_payload.hash_password())?;
    pp(user_payload.create(pool))
}

fn insert_journal(j: &Journal, user_id: Uuid, pool: &db::DbPool) -> Result<()> {
    let mut c = pool.get()?;
    diesel::sql_query("INSERT INTO journals (id,user_id,title,subtitle,content,is_encrypted,last_trace_at,current_draft_id,status,journal_type,sharing_mode,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,NULL,$8,$9,$10,$11,$12)").bind::<SqlUuid,_>(j.id).bind::<SqlUuid,_>(user_id).bind::<Text,_>(&j.title).bind::<Text,_>(&j.subtitle).bind::<Text,_>(&j.content).bind::<Bool,_>(j.is_encrypted).bind::<Nullable<Timestamp>,_>(j.last_trace_at).bind::<Text,_>(j.status.to_db()).bind::<Text,_>(j.journal_type.to_db()).bind::<Text,_>(j.sharing_mode.to_db()).bind::<Timestamp,_>(j.created_at).bind::<Timestamp,_>(j.updated_at).execute(&mut c)?;
    Ok(())
}
fn insert_trace(t: &Trace, user_id: Uuid, pool: &db::DbPool) -> Result<()> {
    let journal_id = t
        .journal_id
        .context("trace without journal_id cannot be replayed")?;
    let mut c = pool.get()?;
    diesel::sql_query("INSERT INTO traces (id,user_id,journal_id,derived_from_trace_id,title,subtitle,content,is_encrypted,encryption_metadata,content_image_asset_id,sharing_sensitivity,timeout_start_at,timeout_at,interaction_date,trace_type,version_integer,status,is_blank,start_writing_at,finalized_at,created_at,updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,CAST($9 AS jsonb),$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22)")
    .bind::<SqlUuid,_>(t.id).bind::<SqlUuid,_>(user_id).bind::<SqlUuid,_>(journal_id).bind::<Nullable<SqlUuid>,_>(None::<Uuid>).bind::<Text,_>(&t.title).bind::<Text,_>(&t.subtitle).bind::<Text,_>(&t.content).bind::<Bool,_>(t.is_encrypted).bind::<Nullable<Text>,_>(t.encryption_metadata.as_ref().map(Value::to_string)).bind::<Nullable<SqlUuid>,_>(None::<Uuid>).bind::<Text,_>(t.sharing_sensitivity.to_db()).bind::<Nullable<Timestamptz>,_>(t.timeout_start_at).bind::<Nullable<Timestamptz>,_>(t.timeout_at).bind::<Timestamp,_>(t.interaction_date).bind::<Text,_>(t.trace_type.to_db()).bind::<diesel::sql_types::Int4,_>(t.version_integer).bind::<Text,_>(t.status.to_db()).bind::<Bool,_>(t.is_blank).bind::<Timestamp,_>(t.start_writing_at).bind::<Nullable<Timestamp>,_>(t.finalized_at).bind::<Timestamp,_>(t.created_at).bind::<Timestamp,_>(t.updated_at).execute(&mut c)?;
    Ok(())
}

fn restore_derived_trace_links(
    traces: &[ExportedTrace],
    imported_ids: &BTreeSet<Uuid>,
    pool: &db::DbPool,
) -> Result<()> {
    let mut connection = pool.get()?;
    for item in traces {
        let Some(source_id) = item.trace.derived_from_trace_id else {
            continue;
        };
        if imported_ids.contains(&source_id) {
            diesel::sql_query("UPDATE traces SET derived_from_trace_id = $2 WHERE id = $1")
                .bind::<SqlUuid, _>(item.trace.id)
                .bind::<SqlUuid, _>(source_id)
                .execute(&mut connection)?;
        }
    }
    Ok(())
}

fn import(file: &Path) -> Result<()> {
    let pool = init()?;
    let value: Value = serde_json::from_slice(&fs::read(file).context("read dataset")?)?;
    if value.get("schema_version").is_none() {
        bail!("expected GET /me/traces/export?format=json all-traces export")
    }
    let export: Export = serde_json::from_value(value)?;
    if export.schema_version != Some(1) {
        bail!("unsupported all-traces export schema")
    }
    if export.traces.iter().any(|x| x.trace.is_encrypted) {
        bail!("encrypted traces cannot be server-side replayed; export plaintext traces")
    }
    let user = synthetic_user(&pool)?;
    let journals: BTreeSet<_> = export.journals.iter().map(|j| j.id).collect();
    let trace_ids: BTreeSet<_> = export.traces.iter().map(|item| item.trace.id).collect();
    for j in &export.journals {
        insert_journal(j, user.id, &pool)?;
    }
    for item in &export.traces {
        let id = item.trace.journal_id.context("trace missing journal_id")?;
        if !journals.contains(&id) {
            bail!("trace {} references missing journal {id}", item.trace.id)
        }
        insert_trace(&item.trace, user.id, &pool)?;
    }
    restore_derived_trace_links(&export.traces, &trace_ids, &pool)?;
    let mut traces = pp(Trace::get_all_for_user(user.id, &pool))?
        .into_iter()
        .filter(|t| t.status == TraceStatus::Finalized)
        .collect::<Vec<_>>();
    traces.sort_by_key(|t| (t.interaction_date, t.created_at, t.id));
    let target = traces.last().context("no finalized trace in export")?;
    let lens = NewLens {
        processing_state: LensProcessingState::InSync,
        fork_landscape_id: None,
        current_landscape_id: None,
        target_trace_id: Some(target.id),
        autoplay: false,
        user_id: user.id,
    }
    .create(&pool)
    .map_err(|err| anyhow!(err.message))?;
    write_json(
        &root().join("import.json"),
        &json!({"dataset": file, "user_id": user.id, "mentor_id": user.mentor_id, "lens_id": lens.id, "target_trace_id": target.id, "trace_count": export.traces.len()}),
    )?;
    println!(
        "imported {} traces; planned lens {}",
        export.traces.len(),
        lens.id
    );
    Ok(())
}

fn imported_lens() -> Result<Uuid> {
    let v: Value =
        serde_json::from_slice(&fs::read(root().join("import.json")).context("run import first")?)?;
    Ok(Uuid::parse_str(
        v["lens_id"].as_str().context("invalid import manifest")?,
    )?)
}

async fn run_one(lens_id: Uuid, pool: &db::DbPool) -> Result<bool> {
    let before = pp(Lens::find_full_lens(lens_id, &pool))?.current_landscape_id;
    let started_at = Utc::now().naive_utc();
    let timer = Instant::now();
    let lens = pp(run_lens_one(lens_id).await)?;
    if lens.current_landscape_id == before {
        return Ok(false);
    }
    let analysis_id = lens
        .current_landscape_id
        .context("lens head disappeared while processing an analysis")?;
    let analysis = pp(
        web_server::entities_v2::landscape_analysis::LandscapeAnalysis::find_full_analysis(
            analysis_id,
            &pool,
        ),
    )?;
    use web_server::entities_v2::landscape_analysis::LandscapeProcessingState;
    if analysis.processing_state != LandscapeProcessingState::Completed {
        bail!("analysis did not complete")
    }
    let artifact = root()
        .join("analyses")
        .join(analysis_id.to_string())
        .join("raw.json");
    let inputs = pp(analysis.get_inputs(&pool))?;
    write_json(
        &artifact,
        &json!({
            "analysis": analysis,
            "lens": lens,
            "inputs": inputs,
            "elements": pp(analysis.get_elements(&pool))?,
            "landmarks": pp(analysis.get_landmarks(None, &pool))?,
            "summaries": pp(AnalysisSummary::find_for_analysis(analysis_id, &pool))?,
            "trace_mirrors": pp(TraceMirror::find_by_landscape_analysis(analysis_id, &pool))?,
            "references": pp(Reference::find_for_landscape_analysis(analysis_id, &pool))?,
            "mentor_feedback": pp(Message::find_latest_feedback_for_analysis(analysis_id, analysis.user_id, &pool))?,
            "llm_calls": pp(LlmCall::get_by_analysis_id(analysis_id, &pool))?
        }),
    )?;
    let checkpoint_policy = policy()?;
    let (dump_path, dump_bytes, dump_sha256) = if checkpoint_policy == "every" {
        let path = root()
            .join("dumps")
            .join(format!("{}-{}.dump", stamp(), analysis_id));
        let (bytes, hash) = dump(&path)?;
        (Some(path), Some(bytes), Some(hash))
    } else {
        (None, None, None)
    };
    let inputs = pp(analysis.get_inputs(&pool))?;
    let manifest = Manifest {
        analysis_id,
        analysis_type: analysis.landscape_analysis_type.to_db().into(),
        status: analysis.processing_state.to_db().into(),
        lens_id,
        parent_analysis_id: analysis.parent_analysis_id,
        lens_head: pp(Lens::find_full_lens(lens_id, &pool))?.current_landscape_id,
        input_trace_ids: inputs.iter().filter_map(|x| x.trace_id).collect(),
        input_trace_mirror_ids: inputs.iter().filter_map(|x| x.trace_mirror_id).collect(),
        started_at,
        completed_at: Utc::now().naive_utc(),
        elapsed_ms: timer.elapsed().as_millis(),
        artifact: artifact.display().to_string(),
        dump_path: dump_path.as_ref().map(|x| x.display().to_string()),
        dump_bytes,
        dump_sha256,
        checkpoint_policy,
    };
    write_json(&artifact.with_file_name("manifest.json"), &manifest)?;
    println!(
        "completed {} ({}) in {}ms",
        manifest.analysis_id, manifest.analysis_type, manifest.elapsed_ms
    );
    if let Some(bytes) = manifest.dump_bytes {
        println!("checkpoint {bytes} bytes (full per-analysis checkpoints grow O(n²); none are auto-deleted)");
    }
    Ok(true)
}

fn parse_run_args(
    mut args: impl Iterator<Item = String>,
    allow_max: bool,
) -> (Option<Uuid>, usize) {
    let mut lens_id = None;
    let mut max = usize::MAX;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--lens" if lens_id.is_none() => {
                lens_id = Some(
                    Uuid::parse_str(&args.next().unwrap_or_else(|| usage()))
                        .unwrap_or_else(|_| usage()),
                );
            }
            "--max" if allow_max && max == usize::MAX => {
                max = args
                    .next()
                    .unwrap_or_else(|| usage())
                    .parse::<usize>()
                    .ok()
                    .filter(|value| *value > 0)
                    .unwrap_or_else(|| usage());
            }
            _ => usage(),
        }
    }
    (lens_id, max)
}

async fn run_command(lens_id: Option<Uuid>, max: usize) -> Result<()> {
    env::set_var("PIPELINE_PLAYGROUND_MODE", "1");
    let pool = init()?;
    let lens_id = lens_id.unwrap_or(imported_lens()?);
    let mut completed = 0usize;
    while completed < max && run_one(lens_id, &pool).await? {
        completed += 1;
    }
    println!("run finished after {completed} completed analyses");
    Ok(())
}

fn branch(from: &Path, name: &str) -> Result<()> {
    let source = db_url()?;
    safe_db(&source)?;
    validate_db_name(name)?;
    if !from.is_file() {
        bail!("checkpoint does not exist")
    }
    let mut target = Url::parse(&source)?;
    target.set_path(&format!("/{name}"));
    let mut admin = Url::parse(&source)?;
    admin.set_path("/postgres");
    let create = Command::new("psql")
        .args([
            "--dbname",
            admin.as_str(),
            "--set=ON_ERROR_STOP=1",
            "--command",
        ])
        .arg(format!("CREATE DATABASE \"{name}\""))
        .status()
        .context("run psql")?;
    if !create.success() {
        bail!("refusing to overwrite branch database")
    }
    let restore = Command::new("pg_restore")
        .args([
            "--clean",
            "--if-exists",
            "--no-owner",
            "--no-privileges",
            "--dbname",
            target.as_str(),
        ])
        .arg(from)
        .status()
        .context("run pg_restore")?;
    if !restore.success() {
        bail!("pg_restore failed; new branch may be incomplete")
    }
    println!("restored into {name}; DATABASE_URL={target}");
    Ok(())
}
fn report() -> Result<()> {
    let dir = root().join("analyses");
    let mut count = 0;
    if dir.exists() {
        for e in fs::read_dir(dir)? {
            let path = e?.path().join("manifest.json");
            if path.exists() {
                let v: Value = serde_json::from_slice(&fs::read(path)?)?;
                println!(
                    "{} {} {}",
                    v["analysis_id"], v["analysis_type"], v["dump_path"]
                );
                count += 1;
            }
        }
    }
    println!("{count} completed analyses");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().unwrap_or_else(|| usage()).as_str() {
        "validate" => {
            let url = db_url()?;
            println!("validated {}", safe_db(&url)?);
        }
        "init" => {
            init()?;
            println!("migrations applied");
        }
        "import" => {
            if args.next().as_deref() != Some("--dataset") {
                usage()
            }
            import(Path::new(&args.next().unwrap_or_else(|| usage())))?;
        }
        "run" => {
            let (lens, _) = parse_run_args(args, false);
            run_command(lens, 1).await?;
        }
        "run-all" => {
            let (lens, max) = parse_run_args(args, true);
            run_command(lens, max).await?;
        }
        "checkpoint" => {
            init()?;
            let label = args.next().unwrap_or_else(stamp);
            validate_checkpoint_label(&label)?;
            let path = root().join("dumps").join(format!("manual-{label}.dump"));
            let (size, hash) = dump(&path)?;
            println!("{} {size} {hash}", path.display());
        }
        "branch" => {
            if args.next().as_deref() != Some("--from") {
                usage()
            }
            let from = PathBuf::from(args.next().unwrap_or_else(|| usage()));
            if args.next().as_deref() != Some("--database") {
                usage()
            }
            branch(&from, &args.next().unwrap_or_else(|| usage()))?;
        }
        "report" => report()?,
        _ => usage(),
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{safe_db, validate_checkpoint_label, validate_db_name};

    #[test]
    fn database_safety_requires_a_strict_pipeline_name() {
        assert!(validate_db_name("pipeline_replay_01").is_ok());
        assert!(safe_db("postgres://localhost/pipeline_replay_01").is_ok());
        for name in [
            "postgres",
            "pipeline_",
            "pipeline_bad-name",
            "pipeline_bad/name",
            "pipeline_bad\";DROP_DATABASE_postgres;--",
        ] {
            assert!(
                validate_db_name(name).is_err(),
                "accepted unsafe name {name}"
            );
        }
        assert!(safe_db("postgres://localhost/postgres").is_err());
        assert!(safe_db("postgres://localhost/pipeline_ok/other").is_err());
    }

    #[test]
    fn checkpoint_labels_cannot_escape_the_dump_directory() {
        assert!(validate_checkpoint_label("after-analysis_42").is_ok());
        for label in ["", "../escape", ".hidden", "with/slash", "two..dots"] {
            assert!(validate_checkpoint_label(label).is_err());
        }
    }
}
