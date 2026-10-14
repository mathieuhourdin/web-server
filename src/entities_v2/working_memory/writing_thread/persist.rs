use super::model::{ThreadView, WritingThread};
use crate::{
    db::DbPool,
    entities_v2::{
        error::{ErrorType, PpdcError},
        trace::Trace,
    },
};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Nullable, Text, Uuid as SqlUuid},
};
use uuid::Uuid;

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
}
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    count: i64,
}
fn err(code: u32, message: &str) -> PpdcError {
    PpdcError::new(code, ErrorType::ApiError, message.to_string())
}
pub fn owned(owner: Uuid, id: Uuid, conn: &mut PgConnection) -> Result<WritingThread, PpdcError> {
    diesel::sql_query("SELECT * FROM writing_threads WHERE id=$1 AND user_id=$2")
        .bind::<SqlUuid, _>(id)
        .bind::<SqlUuid, _>(owner)
        .get_result(conn)
        .optional()?
        .ok_or_else(|| err(404, "Writing thread not found"))
}
const VIEW_FROM: &str =
    " FROM writing_threads w LEFT JOIN traces latest ON latest.id=w.latest_trace_id
    LEFT JOIN traces start ON start.id=w.start_trace_id";
const VIEW_SELECT:&str="SELECT w.*,COALESCE(latest.journal_id,start.journal_id) AS journal_id,
    CASE WHEN latest.status='FINALIZED' THEN latest.title ELSE NULL END AS latest_trace_title,
    (SELECT max(t.finalized_at) FROM writing_thread_trace_ids(w.id) c JOIN traces t ON t.id=c.trace_id WHERE t.status='FINALIZED') AS last_activity_at,
    (SELECT count(*)::bigint FROM writing_thread_trace_ids(w.id) c JOIN traces t ON t.id=c.trace_id WHERE t.status='FINALIZED') AS trace_count,
    COALESCE(latest.status='FINALIZED' AND NOT latest.is_encrypted,FALSE) AS can_continue";
pub fn view(owner: Uuid, id: Uuid, pool: &DbPool) -> Result<ThreadView, PpdcError> {
    diesel::sql_query(format!(
        "{VIEW_SELECT}{VIEW_FROM} WHERE w.id=$1 AND w.user_id=$2"
    ))
    .bind::<SqlUuid, _>(id)
    .bind::<SqlUuid, _>(owner)
    .get_result(&mut pool.get()?)
    .optional()?
    .ok_or_else(|| err(404, "Writing thread not found"))
}
pub fn list(
    owner: Uuid,
    journal: Option<Uuid>,
    trace: Option<Uuid>,
    q: &str,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<ThreadView>, i64), PpdcError> {
    if q.len() > 200 {
        return Err(err(400, "Thread search query is too long"));
    }
    let q = if q.trim().is_empty() {
        None
    } else {
        Some(q.trim())
    };
    let predicate=" WHERE w.user_id=$1 AND ($2::uuid IS NULL OR COALESCE(latest.journal_id,start.journal_id)=$2)
        AND ($3::uuid IS NULL OR EXISTS(SELECT 1 FROM writing_thread_trace_ids(w.id) c WHERE c.trace_id=$3)
            OR w.pending_trace_id=$3)
        AND ($4::text IS NULL OR w.search_vector @@ plainto_tsquery('simple',unaccent($4))
            OR (latest.status='FINALIZED' AND unaccent(COALESCE(latest.title,'')) ILIKE '%'||unaccent($4)||'%'))";
    let mut conn = pool.get()?;
    let total = diesel::sql_query(format!(
        "SELECT count(*)::bigint AS count{VIEW_FROM}{predicate}"
    ))
    .bind::<SqlUuid, _>(owner)
    .bind::<Nullable<SqlUuid>, _>(journal)
    .bind::<Nullable<SqlUuid>, _>(trace)
    .bind::<Nullable<Text>, _>(q)
    .get_result::<Count>(&mut conn)?
    .count;
    let rows = diesel::sql_query(format!(
        "{VIEW_SELECT}{VIEW_FROM}{predicate}
        ORDER BY last_activity_at DESC NULLS LAST,w.created_at DESC,w.id DESC OFFSET $5 LIMIT $6"
    ))
    .bind::<SqlUuid, _>(owner)
    .bind::<Nullable<SqlUuid>, _>(journal)
    .bind::<Nullable<SqlUuid>, _>(trace)
    .bind::<Nullable<Text>, _>(q)
    .bind::<BigInt, _>(offset)
    .bind::<BigInt, _>(limit)
    .load(&mut conn)?;
    Ok((rows, total))
}

pub fn traces(
    owner: Uuid,
    id: Uuid,
    offset: i64,
    limit: i64,
    pool: &DbPool,
) -> Result<(Vec<Trace>, i64), PpdcError> {
    let mut conn = pool.get()?;
    owned(owner, id, &mut conn)?;
    let total=diesel::sql_query("SELECT count(*)::bigint AS count FROM writing_thread_trace_ids($1) c JOIN traces t ON t.id=c.trace_id WHERE t.status='FINALIZED'")
        .bind::<SqlUuid,_>(id).get_result::<Count>(&mut conn)?.count;
    let ids = diesel::sql_query(
        "SELECT t.id FROM writing_thread_trace_ids($1) c JOIN traces t ON t.id=c.trace_id
        WHERE t.status='FINALIZED' ORDER BY c.depth DESC OFFSET $2 LIMIT $3",
    )
    .bind::<SqlUuid, _>(id)
    .bind::<BigInt, _>(offset)
    .bind::<BigInt, _>(limit)
    .load::<Id>(&mut conn)?;
    drop(conn);
    let ids = ids.into_iter().map(|r| r.id).collect::<Vec<_>>();
    let mut by_id = Trace::find_full_traces_by_ids(&ids, pool)?
        .into_iter()
        .map(|t| (t.id, t))
        .collect::<std::collections::HashMap<_, _>>();
    Ok((
        ids.into_iter().filter_map(|id| by_id.remove(&id)).collect(),
        total,
    ))
}

pub fn queue(owner: Uuid, id: Uuid, pool: &DbPool) -> Result<ThreadView, PpdcError> {
    let mut conn = pool.get()?;
    owned(owner, id, &mut conn)?;
    diesel::sql_query("UPDATE writing_threads SET generation_status='PENDING',generation_due_at=NOW(),generation_error=NULL
        WHERE id=$1 AND (compiled_revision IS DISTINCT FROM input_revision OR generation_status IN ('FAILED','DISABLED'))
        AND generation_status<>'RUNNING'") .bind::<SqlUuid,_>(id).execute(&mut conn)?;
    drop(conn);
    view(owner, id, pool)
}
pub fn claim(pool: &DbPool) -> Result<Option<WritingThread>, PpdcError> {
    Ok(diesel::sql_query("WITH candidate AS (
        SELECT id FROM writing_threads WHERE (generation_status='PENDING' AND generation_due_at<=NOW())
            OR (generation_status='RUNNING' AND generation_lease_until<=NOW())
        ORDER BY generation_due_at,id LIMIT 1 FOR UPDATE SKIP LOCKED)
        UPDATE writing_threads w SET generation_status='RUNNING',generation_claim_id=uuid_generate_v4(),
            generation_lease_until=NOW()+INTERVAL '10 minutes' FROM candidate c WHERE w.id=c.id RETURNING w.*")
        .get_result(&mut pool.get()?).optional()?)
}
pub fn finish(
    thread: &WritingThread,
    description: &str,
    tags: &[String],
    pool: &DbPool,
) -> Result<bool, PpdcError> {
    Ok(diesel::sql_query("UPDATE writing_threads SET description=$4,search_tags=$5,
        search_vector=to_tsvector('simple',unaccent($4||' '||array_to_string($5,' '))),
        compiled_revision=input_revision,generation_status='READY',generated_at=NOW(),generation_error=NULL,
        generation_claim_id=NULL,generation_lease_until=NULL
        WHERE id=$1 AND input_revision=$2 AND generation_claim_id=$3")
        .bind::<SqlUuid,_>(thread.id).bind::<BigInt,_>(thread.input_revision)
        .bind::<Nullable<SqlUuid>,_>(thread.generation_claim_id).bind::<Text,_>(description)
        .bind::<Array<Text>,_>(tags).execute(&mut pool.get()?)?>0)
}
pub fn fail(
    thread: &WritingThread,
    status: &str,
    message: &str,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    diesel::sql_query("UPDATE writing_threads SET generation_status=$4,generation_error=$5,
        generation_claim_id=NULL,generation_lease_until=NULL WHERE id=$1 AND input_revision=$2 AND generation_claim_id=$3")
        .bind::<SqlUuid,_>(thread.id).bind::<BigInt,_>(thread.input_revision).bind::<Nullable<SqlUuid>,_>(thread.generation_claim_id)
        .bind::<Text,_>(status).bind::<Text,_>(message).execute(&mut pool.get()?)?;
    Ok(())
}
