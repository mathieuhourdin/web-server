//! Restricted automation clients and delegated demo-user sessions.
//! Existing content handlers keep their normal ownership checks.
use super::{
    error::{ErrorType, PpdcError},
    session::Session,
    user::{User, UserRole},
};
use crate::{
    db::DbPool,
    pagination::{PaginatedResponse, PaginationParams},
};
use axum::{
    body::Body,
    extract::{Extension, Json, Path, Query},
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::{NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Bool, Integer, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const SCOPES: &[&str] = &[
    "demo_users:create",
    "demo_users:read",
    "demo_users:update",
    "demo_users:delete",
    "demo_users:delegate",
    "content:read",
    "content:write",
    "social:read",
    "social:write",
    "ai:use",
];
pub const DEMO_DOMAIN: &str = "@automation.invalid";

fn error(code: u32, message: &str) -> PpdcError {
    PpdcError::new(code, ErrorType::ApiError, message.to_string())
}
#[derive(Clone, Debug, Serialize, QueryableByName)]
pub struct ApiClient {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=Text)]
    pub name: String,
    #[diesel(sql_type=Array<Text>)]
    pub scopes: Vec<String>,
    #[serde(skip_serializing)]
    #[diesel(sql_type=Text)]
    token_hash: String,
    #[diesel(sql_type=Integer)]
    pub max_demo_users: i32,
    #[diesel(sql_type=Integer)]
    pub ai_daily_limit: i32,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub revoked_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Timestamp)]
    pub created_at: NaiveDateTime,
}
#[derive(Clone, Debug)]
pub struct AgentContext {
    pub client: ApiClient,
    pub acting_user_id: Option<Uuid>,
    pub scopes: Vec<String>,
}
impl AgentContext {
    fn require(&self, scope: &str) -> Result<(), PpdcError> {
        if self.scopes.iter().any(|s| s == scope) && self.client.scopes.iter().any(|s| s == scope) {
            Ok(())
        } else {
            Err(error(403, "Automation scope not granted"))
        }
    }
}
#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    count: i64,
}
#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
}
#[derive(QueryableByName)]
struct Flag {
    #[diesel(sql_type=Bool)]
    value: bool,
}
#[derive(QueryableByName)]
struct Delegation {
    #[diesel(sql_type=SqlUuid)]
    api_client_id: Uuid,
    #[diesel(sql_type=Array<Text>)]
    scopes: Vec<String>,
}

pub fn demo_owner(user: Uuid, pool: &DbPool) -> Result<Option<Uuid>, PpdcError> {
    Ok(
        diesel::sql_query("SELECT api_client_id AS id FROM automation_demo_users WHERE user_id=$1")
            .bind::<SqlUuid, _>(user)
            .get_result::<Id>(&mut pool.get()?)
            .optional()?
            .map(|r| r.id),
    )
}
pub fn ensure_same_sandbox(a: Uuid, b: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let allowed = diesel::sql_query("SELECT automation_same_sandbox($1,$2) AS value")
        .bind::<SqlUuid, _>(a)
        .bind::<SqlUuid, _>(b)
        .get_result::<Flag>(&mut pool.get()?)?
        .value;
    if allowed {
        Ok(())
    } else {
        Err(error(
            403,
            "Demo relationships must stay within the same agent sandbox",
        ))
    }
}
pub fn ensure_interaction(a: Uuid, b: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    if demo_owner(a, pool)? == demo_owner(b, pool)? {
        return Ok(());
    }
    // Approved mentors are the only non-demo interaction exception.
    if demo_owner(a, pool)?.is_some()
        && crate::entities_v2::message::routes::is_service_mentor(&User::find(&b, pool)?, pool)?
    {
        return Ok(());
    }
    Err(error(
        403,
        "Demo users cannot interact outside their agent sandbox",
    ))
}
fn client(id: Uuid, conn: &mut PgConnection) -> Result<ApiClient, PpdcError> {
    diesel::sql_query("SELECT * FROM automation_clients WHERE id=$1")
        .bind::<SqlUuid, _>(id)
        .get_result(conn)
        .optional()?
        .ok_or_else(PpdcError::unauthorized)
}
fn ensure_owned(ctx: &AgentContext, user: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    if demo_owner(user, pool)? == Some(ctx.client.id) {
        Ok(())
    } else {
        Err(error(404, "Demo user not found"))
    }
}
pub fn delegated_context(
    session: &Session,
    pool: &DbPool,
) -> Result<Option<AgentContext>, PpdcError> {
    let Some(user) = session.user_id else {
        return Ok(None);
    };
    let Some(owner) = demo_owner(user, pool)? else {
        return Ok(None);
    };
    let mut conn = pool.get()?;
    let d=diesel::sql_query("SELECT api_client_id,scopes FROM automation_delegations WHERE session_id=$1 AND user_id=$2")
        .bind::<SqlUuid,_>(session.id).bind::<SqlUuid,_>(user).get_result::<Delegation>(&mut conn).optional()?
        .ok_or_else(PpdcError::unauthorized)?;
    let client = client(d.api_client_id, &mut conn)?;
    if client.id != owner || client.revoked_at.is_some() {
        return Err(PpdcError::unauthorized());
    }
    Ok(Some(AgentContext {
        client,
        acting_user_id: Some(user),
        scopes: d.scopes,
    }))
}
fn authenticate(header: &str, pool: &DbPool) -> Result<AgentContext, PpdcError> {
    let raw = header
        .strip_prefix("Bearer agent.")
        .ok_or_else(PpdcError::unauthorized)?;
    let (id, secret) = raw.split_once('.').ok_or_else(PpdcError::unauthorized)?;
    let id = Uuid::parse_str(id).map_err(|_| PpdcError::unauthorized())?;
    let c = client(id, &mut *pool.get()?)?;
    if c.revoked_at.is_some() || !Session::verify_secret(&c.token_hash, secret)? {
        return Err(PpdcError::unauthorized());
    }
    Ok(AgentContext {
        scopes: c.scopes.clone(),
        client: c,
        acting_user_id: None,
    })
}

// Exact patterns: adding an ordinary route never implicitly grants agent access.
// IDs must be UUIDs; path normalization tricks and arbitrary subroutes are denied.
const DELEGATED_ROUTES: &[(&str, &str, &str)] = &[
    ("GET", "/me/feed", "content:read"),
    ("GET", "/me/traces", "content:read"),
    ("GET", "/me/unread_counts", "content:read"),
    ("GET", "/me/wal", "content:read"),
    ("POST", "/me/wal", "content:write"),
    ("GET", "/me/wal/compilation", "content:read"),
    ("POST", "/me/wal/compilation", "content:write"),
    ("GET", "/users/search", "social:read"),
    ("GET", "/users/:id", "social:read"),
    ("GET", "/users/:id/journals", "content:read"),
    ("GET", "/users/:id/traces", "content:read"),
    ("GET", "/users/:id/albums", "content:read"),
    ("GET", "/users/:id/documents", "content:read"),
    ("GET", "/mentors", "social:read"),
    ("POST", "/journals", "content:write"),
    ("GET", "/journals/:id", "content:read"),
    ("PUT", "/journals/:id", "content:write"),
    ("GET", "/journals/:id/traces", "content:read"),
    ("GET", "/journals/:id/draft", "content:read"),
    ("POST", "/journals/:id/traces/finalized", "content:write"),
    ("POST", "/journals/:id/draft", "content:write"),
    ("PATCH", "/journals/:id/draft", "content:write"),
    ("POST", "/journals/:id/messages", "social:write"),
    ("GET", "/traces/:id", "content:read"),
    ("PATCH", "/traces/:id", "content:write"),
    ("PUT", "/traces/:id", "content:write"),
    ("DELETE", "/traces/:id", "content:write"),
    ("POST", "/traces/:id/messages", "social:write"),
    ("GET", "/traces/:id/seen_by", "content:read"),
    ("PUT", "/traces/:id/seen", "social:write"),
    ("POST", "/traces/:id/complements", "content:write"),
    ("GET", "/traces/:id/complements", "content:read"),
    ("PUT", "/traces/:id/post", "content:write"),
    ("GET", "/traces/:id/post", "content:read"),
    ("GET", "/posts/feed", "content:read"),
    ("GET", "/posts/:id", "content:read"),
    ("POST", "/posts/:id/messages", "social:write"),
    ("GET", "/posts/:id/grants", "social:read"),
    ("POST", "/posts/:id/grants", "social:write"),
    ("DELETE", "/posts/:id/grants/:id", "social:write"),
    ("GET", "/messages", "social:read"),
    ("POST", "/messages", "social:write"),
    ("GET", "/messages/:id", "social:read"),
    ("PUT", "/messages/:id", "social:write"),
    ("GET", "/relationships", "social:read"),
    ("POST", "/relationships", "social:write"),
    ("PUT", "/relationships/:id", "social:write"),
    ("DELETE", "/relationships/:id", "social:write"),
    ("GET", "/relationships/followers", "social:read"),
    ("GET", "/relationships/following", "social:read"),
    ("GET", "/relationships/requests/incoming", "social:read"),
    ("GET", "/relationships/requests/outgoing", "social:read"),
    ("GET", "/audiences", "social:read"),
    ("POST", "/audiences", "social:write"),
    ("GET", "/audiences/:id", "social:read"),
    ("PATCH", "/audiences/:id", "social:write"),
    ("GET", "/audiences/:id/members", "social:read"),
    ("PUT", "/audiences/:id/members/:id", "social:write"),
    (
        "POST",
        "/audiences/:id/members/:id/history_decision",
        "social:write",
    ),
    (
        "GET",
        "/audiences/:id/members/:id/history_summary",
        "social:read",
    ),
    ("PUT", "/posts/:id/audiences/:id", "social:write"),
    ("DELETE", "/posts/:id/audiences/:id", "social:write"),
    ("POST", "/albums", "content:write"),
    ("GET", "/albums/:id", "content:read"),
    ("PUT", "/albums/:id", "content:write"),
    ("GET", "/journals/:id/messages", "social:read"),
    ("GET", "/traces/:id/messages", "social:read"),
    ("GET", "/messages/conversations", "social:read"),
    ("GET", "/messages/conversations/:id", "social:read"),
    ("GET", "/audiences/:id/posts", "social:read"),
    ("GET", "/audiences/history_reviews", "social:read"),
    ("GET", "/audiences/:id/members/:id", "social:read"),
    ("PATCH", "/audiences/:id/members/:id", "social:write"),
    ("DELETE", "/audiences/:id/members/:id", "social:write"),
    ("GET", "/journals/:id/default_audiences", "social:read"),
    ("PUT", "/journals/:id/default_audiences", "social:write"),
    ("POST", "/posts", "content:write"),
    ("PUT", "/posts/:id", "content:write"),
    ("GET", "/albums/:id/items", "content:read"),
    ("POST", "/albums/:id/items", "content:write"),
];
fn required_scope(method: &str, path: &str) -> Option<&'static str> {
    DELEGATED_ROUTES.iter().find_map(|(m, pattern, scope)| {
        let a = path.trim_end_matches('/').split('/').collect::<Vec<_>>();
        let b = pattern.split('/').collect::<Vec<_>>();
        (*m == method
            && a.len() == b.len()
            && a.iter().zip(b).all(|(a, b)| {
                if b == ":id" {
                    Uuid::parse_str(a).is_ok()
                } else {
                    *a == b
                }
            }))
        .then_some(*scope)
    })
}
pub async fn gate(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let context = if header.starts_with("Bearer agent.") {
        authenticate(header, &pool).map(Some)
    } else {
        delegated_context(&session, &pool)
    };
    let ctx = match context {
        Ok(Some(c)) => c,
        Ok(None) => {
            if req.uri().path().starts_with("/automation/") {
                return PpdcError::unauthorized().into_response();
            }
            return next.run(req).await;
        }
        Err(e) => return e.into_response(),
    };
    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_string();
    let mut permitted = if ctx.acting_user_id.is_none() {
        path.starts_with("/automation/") || path == "/me/capabilities"
    } else if path == "/me/capabilities" {
        true
    } else {
        required_scope(&method, &path)
            .map(|s| ctx.require(s).is_ok())
            .unwrap_or(false)
    };
    // Content writes can enqueue automatic analysis/compilation. Do not let a
    // narrower delegated credential bypass ai:use through those background jobs.
    if permitted && required_scope(&method, &path) == Some("content:write") {
        if let Some(user) = ctx.acting_user_id {
            match User::find(&user, &pool) {
                Ok(user) if user.allows_ai_features() => permitted = ctx.require("ai:use").is_ok(),
                Ok(_) => {}
                Err(e) => return e.into_response(),
            }
        }
    }
    if !permitted {
        let _ = audit(&ctx, &format!("{method} {path}"), 403, &pool);
        return error(403, "Route not allowed for this automation credential").into_response();
    }
    req.extensions_mut().insert(ctx.clone());
    let response = next.run(req).await;
    let action = format!("{method} {path}");
    if let Err(e) = audit(&ctx, &action, response.status().as_u16(), &pool) {
        tracing::warn!(target:"automation",api_client_id=%ctx.client.id,error=%e.message,"automation_audit_failed");
    }
    response
}
fn audit(ctx: &AgentContext, action: &str, status: u16, pool: &DbPool) -> Result<(), PpdcError> {
    diesel::sql_query("INSERT INTO automation_audit_events(api_client_id,acting_user_id,action,status_code) VALUES($1,$2,$3,$4)")
        .bind::<SqlUuid,_>(ctx.client.id).bind::<Nullable<SqlUuid>,_>(ctx.acting_user_id)
        .bind::<Text,_>(action).bind::<Integer,_>(status as i32).execute(&mut pool.get()?)?;
    Ok(())
}
fn management(ctx: &AgentContext, scope: &str) -> Result<(), PpdcError> {
    if ctx.acting_user_id.is_some() {
        return Err(error(
            403,
            "Use the agent credential for demo-account management",
        ));
    }
    ctx.require(scope)
}
fn admin(session: &Session, pool: &DbPool) -> Result<(), PpdcError> {
    let user = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    if User::find(&user, pool)?.has_role(UserRole::Admin, pool)? {
        Ok(())
    } else {
        Err(error(403, "Admin role required"))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateClient {
    name: String,
    scopes: Vec<String>,
    #[serde(default = "default_max")]
    max_demo_users: i32,
    #[serde(default = "default_max")]
    ai_daily_limit: i32,
}
fn default_max() -> i32 {
    100
}
#[derive(Serialize)]
pub struct ClientToken {
    client: ApiClient,
    token: String,
}
pub async fn create_client(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(p): Json<CreateClient>,
) -> Result<Json<ClientToken>, PpdcError> {
    admin(&session, &pool)?;
    if p.name.trim().is_empty()
        || p.name.len() > 100
        || p.scopes.is_empty()
        || p.scopes.iter().any(|s| !SCOPES.contains(&s.as_str()))
        || !(1..=1000).contains(&p.max_demo_users)
        || !(0..=1000).contains(&p.ai_daily_limit)
    {
        return Err(error(400, "Invalid client name, scopes or limits"));
    }
    let secret = Session::generate_secret();
    let hash = Session::hash_secret(&secret)?;
    let c=diesel::sql_query("INSERT INTO automation_clients(name,scopes,token_hash,max_demo_users,ai_daily_limit) VALUES($1,$2,$3,$4,$5) RETURNING *")
        .bind::<Text,_>(p.name.trim()).bind::<Array<Text>,_>(p.scopes).bind::<Text,_>(hash)
        .bind::<Integer,_>(p.max_demo_users).bind::<Integer,_>(p.ai_daily_limit).get_result::<ApiClient>(&mut pool.get()?)?;
    tracing::info!(target:"automation",admin_user_id=?session.user_id,api_client_id=%c.id,"automation_client_created");
    Ok(Json(ClientToken {
        token: format!("Bearer agent.{}.{}", c.id, secret),
        client: c,
    }))
}
pub async fn list_clients(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(q): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<ApiClient>>, PpdcError> {
    admin(&session, &pool)?;
    let p = q.validate()?;
    let mut conn = pool.get()?;
    let total = diesel::sql_query("SELECT count(*)::bigint AS count FROM automation_clients")
        .get_result::<Count>(&mut conn)?
        .count;
    let rows = diesel::sql_query(
        "SELECT * FROM automation_clients ORDER BY created_at,id OFFSET $1 LIMIT $2",
    )
    .bind::<BigInt, _>(p.offset)
    .bind::<BigInt, _>(p.limit)
    .load(&mut conn)?;
    Ok(Json(PaginatedResponse::new(rows, p, total)))
}
pub async fn rotate_client(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<ClientToken>, PpdcError> {
    admin(&session, &pool)?;
    let secret = Session::generate_secret();
    let hash = Session::hash_secret(&secret)?;
    let mut conn = pool.get()?;
    let c=conn.transaction::<_,PpdcError,_>(|conn| {
        let c=diesel::sql_query("UPDATE automation_clients SET token_hash=$2 WHERE id=$1 AND revoked_at IS NULL RETURNING *")
            .bind::<SqlUuid,_>(id).bind::<Text,_>(hash).get_result::<ApiClient>(conn).optional()?.ok_or_else(||error(404,"Active client not found"))?;
        diesel::sql_query("UPDATE sessions SET revoked_at=NOW() WHERE id IN (SELECT session_id FROM automation_delegations WHERE api_client_id=$1)")
            .bind::<SqlUuid,_>(id).execute(conn)?;Ok(c)
    })?;
    tracing::info!(target:"automation",admin_user_id=?session.user_id,api_client_id=%id,"automation_client_rotated");
    Ok(Json(ClientToken {
        token: format!("Bearer agent.{id}.{secret}"),
        client: c,
    }))
}
pub async fn revoke_client(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiClient>, PpdcError> {
    admin(&session, &pool)?;
    let c=diesel::sql_query("UPDATE automation_clients SET revoked_at=COALESCE(revoked_at,NOW()) WHERE id=$1 RETURNING *")
        .bind::<SqlUuid,_>(id).get_result::<ApiClient>(&mut pool.get()?).optional()?.ok_or_else(||error(404,"Client not found"))?;
    tracing::info!(target:"automation",admin_user_id=?session.user_id,api_client_id=%id,"automation_client_revoked");
    Ok(Json(c))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoInput {
    first_name: String,
    last_name: String,
    handle: String,
    biography: Option<String>,
    timezone: Option<String>,
    #[serde(default)]
    ai_features_enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoBatch {
    users: Vec<DemoInput>,
}
#[derive(Serialize)]
pub struct DemoView {
    pub account_kind: &'static str,
    pub created_by_api_client_id: Uuid,
    pub user: User,
}
fn demo_view(user: User, ctx: &AgentContext) -> DemoView {
    DemoView {
        account_kind: "demo",
        created_by_api_client_id: ctx.client.id,
        user,
    }
}
fn validate_demo(p: &DemoInput) -> Result<(), PpdcError> {
    if p.first_name.trim().is_empty()
        || p.first_name.len() > 100
        || p.last_name.len() > 100
        || p.biography.as_ref().is_some_and(|v| v.len() > 5000)
    {
        return Err(error(400, "Invalid demo profile"));
    }
    if p.handle.is_empty()
        || p.handle.len() > 50
        || !p
            .handle
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@_.-".contains(c))
    {
        return Err(error(400, "Invalid handle"));
    }
    if let Some(tz) = &p.timezone {
        if tz.parse::<chrono_tz::Tz>().is_err() {
            return Err(error(400, "Invalid timezone"));
        }
    }
    Ok(())
}
pub async fn create_demos(
    Extension(pool): Extension<DbPool>,
    Extension(ctx): Extension<AgentContext>,
    Json(p): Json<DemoBatch>,
) -> Result<Json<Vec<DemoView>>, PpdcError> {
    management(&ctx, "demo_users:create")?;
    if p.users.is_empty() || p.users.len() > 20 {
        return Err(error(400, "Create between 1 and 20 demo users per batch"));
    }
    for user in &p.users {
        validate_demo(user)?;
        if user.ai_features_enabled {
            ctx.require("ai:use")?;
        }
    }
    let hash = Session::hash_secret(&Session::generate_secret())?;
    let mut conn = pool.get()?;
    let ids=conn.transaction::<_,PpdcError,_>(|conn| {
        // Serialize quota checks with other batches and token rotation/revocation.
        diesel::sql_query("SELECT id FROM automation_clients WHERE id=$1 AND revoked_at IS NULL FOR UPDATE")
            .bind::<SqlUuid,_>(ctx.client.id).get_result::<Id>(conn).optional()?.ok_or_else(PpdcError::unauthorized)?;
        let count=diesel::sql_query("SELECT count(*)::bigint AS count FROM automation_demo_users WHERE api_client_id=$1")
            .bind::<SqlUuid,_>(ctx.client.id).get_result::<Count>(conn)?.count;
        if count+p.users.len() as i64>ctx.client.max_demo_users as i64 {return Err(error(429,"Demo account quota reached"));}
        let mut ids=Vec::new();
        for p in &p.users {
            let id=Uuid::new_v4();
            diesel::sql_query("INSERT INTO users(id,email,password,first_name,last_name,handle,principal_type,is_platform_user,biography,timezone,
                shared_journal_activity_email_mode,received_message_email_mode,shared_journal_weekly_digest_enabled,mentor_feedback_email_enabled,
                ai_features_enabled,ai_features_enabled_by_admin,updated_at)
                VALUES($1,$2,$3,$4,$5,$6,'HUMAN',TRUE,$7,$8,'off','off',FALSE,FALSE,$9,TRUE,NOW())")
                .bind::<SqlUuid,_>(id).bind::<Text,_>(format!("demo-{id}{DEMO_DOMAIN}")).bind::<Text,_>(&hash)
                .bind::<Text,_>(p.first_name.trim()).bind::<Text,_>(p.last_name.trim()).bind::<Text,_>(&p.handle)
                .bind::<Nullable<Text>,_>(&p.biography).bind::<Text,_>(p.timezone.as_deref().unwrap_or("Europe/Paris"))
                .bind::<Bool,_>(p.ai_features_enabled).execute(conn)?;
            diesel::sql_query("INSERT INTO automation_demo_users(user_id,api_client_id) VALUES($1,$2)")
                .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(ctx.client.id).execute(conn)?;
            diesel::sql_query("INSERT INTO user_roles(user_id,role) VALUES($1,'MEMBER')")
                .bind::<SqlUuid,_>(id).execute(conn)?;
            ids.push(id);
        }Ok(ids)
    })?;
    Ok(Json(
        User::find_many(&ids, &pool)?
            .into_iter()
            .map(|u| demo_view(u, &ctx))
            .collect(),
    ))
}
pub async fn list_demos(
    Extension(pool): Extension<DbPool>,
    Extension(ctx): Extension<AgentContext>,
    Query(q): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<DemoView>>, PpdcError> {
    management(&ctx, "demo_users:read")?;
    let p = q.validate()?;
    let mut conn = pool.get()?;
    let total = diesel::sql_query(
        "SELECT count(*)::bigint AS count FROM automation_demo_users WHERE api_client_id=$1",
    )
    .bind::<SqlUuid, _>(ctx.client.id)
    .get_result::<Count>(&mut conn)?
    .count;
    let ids=diesel::sql_query("SELECT user_id AS id FROM automation_demo_users WHERE api_client_id=$1 ORDER BY created_at,id OFFSET $2 LIMIT $3")
        .bind::<SqlUuid,_>(ctx.client.id).bind::<BigInt,_>(p.offset).bind::<BigInt,_>(p.limit).load::<Id>(&mut conn)?;
    drop(conn);
    let users = ids
        .into_iter()
        .map(|r| User::find(&r.id, &pool).map(|u| demo_view(u, &ctx)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(PaginatedResponse::new(users, p, total)))
}
pub async fn get_demo(
    Extension(pool): Extension<DbPool>,
    Extension(ctx): Extension<AgentContext>,
    Path(user): Path<Uuid>,
) -> Result<Json<DemoView>, PpdcError> {
    management(&ctx, "demo_users:read")?;
    ensure_owned(&ctx, user, &pool)?;
    Ok(Json(demo_view(User::find(&user, &pool)?, &ctx)))
}
pub async fn update_demo(
    Extension(pool): Extension<DbPool>,
    Extension(ctx): Extension<AgentContext>,
    Path(user): Path<Uuid>,
    Json(p): Json<DemoInput>,
) -> Result<Json<DemoView>, PpdcError> {
    management(&ctx, "demo_users:update")?;
    ensure_owned(&ctx, user, &pool)?;
    validate_demo(&p)?;
    if p.ai_features_enabled {
        ctx.require("ai:use")?;
    }
    diesel::sql_query("UPDATE users SET first_name=$2,last_name=$3,handle=$4,biography=$5,timezone=$6,ai_features_enabled=$7,updated_at=NOW() WHERE id=$1")
        .bind::<SqlUuid,_>(user).bind::<Text,_>(p.first_name.trim()).bind::<Text,_>(p.last_name.trim()).bind::<Text,_>(p.handle)
        .bind::<Nullable<Text>,_>(p.biography).bind::<Text,_>(p.timezone.as_deref().unwrap_or("Europe/Paris"))
        .bind::<Bool,_>(p.ai_features_enabled).execute(&mut pool.get()?)?;
    Ok(Json(demo_view(User::find(&user, &pool)?, &ctx)))
}
pub async fn delete_demo(
    Extension(pool): Extension<DbPool>,
    Extension(ctx): Extension<AgentContext>,
    Path(user): Path<Uuid>,
) -> Result<StatusCode, PpdcError> {
    management(&ctx, "demo_users:delete")?;
    ensure_owned(&ctx, user, &pool)?;
    super::user::purge_user(user, "AUTOMATION_DEMO_DELETED", &pool).await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelegateInput {
    scopes: Vec<String>,
}
#[derive(Serialize)]
pub struct DelegatedToken {
    user_id: Uuid,
    api_client_id: Uuid,
    token: String,
    expires_at: NaiveDateTime,
    scopes: Vec<String>,
}
pub async fn delegate(
    Extension(pool): Extension<DbPool>,
    Extension(ctx): Extension<AgentContext>,
    Path(user): Path<Uuid>,
    Json(p): Json<DelegateInput>,
) -> Result<Json<DelegatedToken>, PpdcError> {
    management(&ctx, "demo_users:delegate")?;
    ensure_owned(&ctx, user, &pool)?;
    if p.scopes.is_empty()
        || p.scopes.iter().any(|s| {
            !matches!(
                s.as_str(),
                "content:read" | "content:write" | "social:read" | "social:write" | "ai:use"
            )
        })
    {
        return Err(error(400, "Invalid delegation scopes"));
    }
    for scope in &p.scopes {
        ctx.require(scope)?;
    }
    let (session, token) = Session::create_delegated(user, &pool)?;
    let expires_at = Utc::now().naive_utc() + chrono::Duration::hours(1);
    let mut conn = pool.get()?;
    conn.transaction::<_,PpdcError,_>(|conn| {
        diesel::sql_query("INSERT INTO automation_delegations(session_id,api_client_id,user_id,scopes) VALUES($1,$2,$3,$4)")
            .bind::<SqlUuid,_>(session.id).bind::<SqlUuid,_>(ctx.client.id).bind::<SqlUuid,_>(user).bind::<Array<Text>,_>(&p.scopes).execute(conn)?;
        diesel::sql_query("UPDATE sessions SET expires_at=$2 WHERE id=$1").bind::<SqlUuid,_>(session.id).bind::<Timestamp,_>(expires_at).execute(conn)?;
        Ok(())
    })?;
    Ok(Json(DelegatedToken {
        user_id: user,
        api_client_id: ctx.client.id,
        token,
        expires_at,
        scopes: p.scopes,
    }))
}
pub async fn capabilities(ctx: Option<Extension<AgentContext>>) -> Result<Json<Value>, PpdcError> {
    let ctx = ctx.ok_or_else(PpdcError::unauthorized)?.0;
    Ok(Json(
        json!({"api_client_id":ctx.client.id,"acting_user_id":ctx.acting_user_id,"scopes":ctx.scopes,
        "max_demo_users":ctx.client.max_demo_users,"batch_limit":20,"ai_daily_limit":ctx.client.ai_daily_limit,
        "delegation_ttl_seconds":3600,"sandbox":"same_agent_only","normal_login":false,"public_sharing":false,
        "mentor_editing":false,"contract_url":"/automation/contract"}),
    ))
}
pub async fn contract(Extension(_ctx): Extension<AgentContext>) -> Json<Value> {
    Json(
        json!({"version":1,"scopes":SCOPES,"delegated_routes":DELEGATED_ROUTES.iter().map(|(method,path,scope)|json!({"method":method,"path":path,"scope":scope})).collect::<Vec<_>>(),
        "automation_contract_markdown":include_str!("../../../doc/automation_api.md"),
        "platform_contract_markdown":include_str!("../../../doc/api_quick.md")}),
    )
}

/// Atomic request-budget reservation, shared across all demo users of an agent.
/// Failed requests consume quota too; this is a request limit, not a dollar budget.
pub fn reserve_ai(
    user: Uuid,
    session_id: Option<Uuid>,
    kind: &str,
    pool: &DbPool,
) -> Result<(), PpdcError> {
    let Some(owner) = demo_owner(user, pool)? else {
        return Ok(());
    };
    if let Some(id) = session_id {
        let session = Session::find(&id, pool)?;
        if let Some(ctx) = delegated_context(&session, pool)? {
            ctx.require("ai:use")?;
        } else {
            return Err(PpdcError::unauthorized());
        }
    }
    let mut conn = pool.get()?;
    conn.transaction::<_,PpdcError,_>(|conn| {
        diesel::sql_query("SELECT id FROM automation_clients WHERE id=$1 FOR UPDATE").bind::<SqlUuid,_>(owner).get_result::<Id>(conn)?;
        let c=client(owner,conn)?;
        if c.revoked_at.is_some() || !c.scopes.iter().any(|s|s=="ai:use") {return Err(error(403,"Agent AI access disabled"));}
        let count=diesel::sql_query("SELECT count(*)::bigint AS count FROM automation_ai_requests WHERE api_client_id=$1 AND created_at>NOW()-INTERVAL '24 hours'")
            .bind::<SqlUuid,_>(owner).get_result::<Count>(conn)?.count;
        if count>=c.ai_daily_limit as i64 {return Err(error(429,"Agent daily AI request quota reached"));}
        diesel::sql_query("INSERT INTO automation_ai_requests(api_client_id,user_id,request_kind) VALUES($1,$2,$3)")
            .bind::<SqlUuid,_>(owner).bind::<SqlUuid,_>(user).bind::<Text,_>(kind).execute(conn)?;Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    #[ignore = "Requires an explicitly configured disposable PostgreSQL database"]
    async fn automation_sandbox_delegation_and_delivery_integration() {
        use diesel::{connection::SimpleConnection, r2d2::ConnectionManager};
        let url = std::env::var("AUDIENCE_TEST_DATABASE_URL").unwrap();
        assert_eq!(url::Url::parse(&url).unwrap().path(), "/audience_test");
        let pool = DbPool::builder()
            .max_size(8)
            .build(ConnectionManager::new(url))
            .unwrap();
        let admin_id = Uuid::new_v4();
        let real = Uuid::new_v4();
        let mentor = Uuid::new_v4();
        pool.get()
            .unwrap()
            .batch_execute(&format!(
                "INSERT INTO users(id,email,first_name,last_name,handle,password,updated_at) VALUES
            ('{admin_id}','{admin_id}@test.invalid','Admin','Test','@{admin_id}','unused',NOW()),
            ('{real}','{real}@test.invalid','Real','Test','@{real}','unused',NOW()),
            ('{mentor}','{mentor}@test.invalid','Mentor','Test','@{mentor}','unused',NOW());
            UPDATE users SET principal_type='SERVICE' WHERE id='{mentor}';
            INSERT INTO user_roles(user_id,role) VALUES('{admin_id}','ADMIN');"
            ))
            .unwrap();
        let mut admin = Session::anonymous();
        admin.user_id = Some(admin_id);
        let key = create_client(
            Extension(pool.clone()),
            Extension(admin.clone()),
            Json(CreateClient {
                name: "Demo integration".into(),
                scopes: SCOPES.iter().map(|s| s.to_string()).collect(),
                max_demo_users: 2,
                ai_daily_limit: 1,
            }),
        )
        .await
        .unwrap()
        .0;
        let ctx = authenticate(&key.token, &pool).unwrap();
        let app = crate::router::create_router().layer(Extension(pool.clone()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let http = reqwest::Client::new();
        let response=http.post(format!("{base}/automation/demo_users")).header("Authorization",&key.token)
            .json(&json!({"users":[{"first_name":"Demo","last_name":"One","handle":format!("demo-{}",Uuid::new_v4())},
                {"first_name":"Demo","last_name":"Two","handle":format!("demo-{}",Uuid::new_v4())}]})).send().await.unwrap();
        let status = response.status().as_u16();
        let body = response.text().await.unwrap();
        assert_eq!(status, 200, "{body}");
        let rows = serde_json::from_str::<Vec<Value>>(&body).unwrap();
        let a = Uuid::parse_str(rows[0]["user"]["id"].as_str().unwrap()).unwrap();
        let b = Uuid::parse_str(rows[1]["user"]["id"].as_str().unwrap()).unwrap();
        assert!(
            Session::create_authenticated(a, &pool).is_err(),
            "No normal login session for demos"
        );
        assert!(ensure_same_sandbox(a, b, &pool).is_ok());
        assert!(ensure_same_sandbox(a, real, &pool).is_err());
        assert!(ensure_same_sandbox(real, a, &pool).is_err());
        assert!(
            ensure_interaction(a, mentor, &pool).is_ok(),
            "Existing service mentors need no MENTOR role row"
        );
        let key2 = create_client(
            Extension(pool.clone()),
            Extension(admin.clone()),
            Json(CreateClient {
                name: "Other sandbox".into(),
                scopes: ctx.scopes.clone(),
                max_demo_users: 2,
                ai_daily_limit: 0,
            }),
        )
        .await
        .unwrap()
        .0;
        let ctx2 = authenticate(&key2.token, &pool).unwrap();
        let other = create_demos(
            Extension(pool.clone()),
            Extension(ctx2.clone()),
            Json(DemoBatch {
                users: vec![DemoInput {
                    first_name: "Other".into(),
                    last_name: "Demo".into(),
                    handle: format!("other-{}", Uuid::new_v4()),
                    biography: None,
                    timezone: None,
                    ai_features_enabled: false,
                }],
            }),
        )
        .await
        .unwrap()
        .0
        .remove(0)
        .user
        .id;
        assert!(ensure_same_sandbox(a, other, &pool).is_err());
        assert!(ensure_owned(&ctx, a, &pool).is_ok());
        assert!(ensure_owned(&ctx2, a, &pool).is_err());
        let d = delegate(
            Extension(pool.clone()),
            Extension(ctx.clone()),
            Path(a),
            Json(DelegateInput {
                scopes: vec!["content:read".into(), "social:read".into()],
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(Session::get_valid_session_from_authorization(&d.token, &pool).is_ok());
        assert_eq!(
            http.get(format!("{base}/me/feed"))
                .header("Authorization", &d.token)
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            200
        );
        assert_eq!(
            http.post(format!("{base}/journals"))
                .header("Authorization", &d.token)
                .json(&json!({"title":"Forbidden"}))
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            403
        );
        assert_eq!(
            http.post(format!("{base}/sessions"))
                .header("Authorization", &d.token)
                .json(&json!({}))
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            403
        );
        assert_eq!(
            http.get(format!("{base}/me/capabilities"))
                .header("Authorization", &key.token)
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            200
        );
        assert_eq!(
            http.get(format!("{base}/automation/contract"))
                .header("Authorization", &key.token)
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            200
        );
        assert!(pool.get().unwrap().batch_execute(&format!("INSERT INTO relationships(requester_user_id,target_user_id,relationship_type,status) VALUES('{a}','{real}','FOLLOW','ACCEPTED')")).is_err());
        assert!(pool
            .get()
            .unwrap()
            .batch_execute(&format!(
                "UPDATE users SET email='real@test.invalid' WHERE id='{a}'"
            ))
            .is_err());
        assert!(pool
            .get()
            .unwrap()
            .batch_execute(&format!(
                "INSERT INTO user_roles(user_id,role) VALUES('{a}','ADMIN')"
            ))
            .is_err());
        let discover = diesel::sql_query("SELECT automation_user_discoverable($1,$2) AS value")
            .bind::<SqlUuid, _>(a)
            .bind::<SqlUuid, _>(real)
            .get_result::<Flag>(&mut pool.get().unwrap())
            .unwrap();
        assert!(!discover.value);
        let journal = Uuid::new_v4();
        let trace = Uuid::new_v4();
        let post = Uuid::new_v4();
        pool.get().unwrap().batch_execute(&format!("INSERT INTO journals(id,user_id,title,subtitle,content,journal_type,status)
            VALUES('{journal}','{real}','Public test','','','USER_JOURNAL','ACTIVE');
            INSERT INTO traces(id,user_id,journal_id,title,subtitle,content,interaction_date,trace_type,status,start_writing_at,finalized_at)
            VALUES('{trace}','{real}','{journal}','Public test','','Test',NOW(),'USER_TRACE','FINALIZED',NOW(),NOW());
            INSERT INTO posts(id,user_id,source_trace_id,title,subtitle,content,interaction_type,post_type,status,publishing_date)
            VALUES('{post}','{real}','{trace}','','','','OUTPUT','IDEA','PUBLISHED',NOW());
            INSERT INTO post_grants(id,post_id,owner_user_id,grantee_scope,access_level,status)
            VALUES(uuid_generate_v4(),'{post}','{real}','ALL_PLATFORM_USERS','READ','ACTIVE');")).unwrap();
        assert!(
            !crate::entities_v2::post_grant::PostGrant::find_shared_post_ids_for_user(a, &pool)
                .unwrap()
                .contains(&post)
        );
        assert!(
            !crate::entities_v2::post_grant::PostGrant::user_can_read_post(
                &crate::entities_v2::post::Post::find_full(post, &pool).unwrap(),
                a,
                &pool
            )
            .unwrap()
        );
        let email = super::super::mailer::NewOutboundEmail::new(
            Some(a),
            "DEMO_TEST".into(),
            None,
            None,
            User::find(&a, &pool).unwrap().email,
            "test@test.invalid".into(),
            "Suppressed".into(),
            Some("Test".into()),
            None,
            super::super::mailer::OutboundEmailProvider::Resend,
            None,
        )
        .create(&pool)
        .unwrap();
        assert_eq!(email.status, "SUPPRESSED");
        assert_eq!(
            super::super::push::send_to_mobile_user(
                a,
                super::super::push::PushNotification {
                    title: "Test".into(),
                    body: "Test".into(),
                    data: Default::default(),
                    thread_id: None,
                },
                &pool
            )
            .await
            .unwrap()
            .sent_count,
            0
        );
        assert!(reserve_ai(a, None, "test", &pool).is_ok());
        assert_eq!(
            reserve_ai(b, None, "test", &pool).unwrap_err().status_code,
            429
        );
        let _ = rotate_client(
            Extension(pool.clone()),
            Extension(admin.clone()),
            Path(ctx.client.id),
        )
        .await
        .unwrap();
        assert!(authenticate(&key.token, &pool).is_err());
        assert!(Session::get_valid_session_from_authorization(&d.token, &pool).is_err());
        assert_eq!(
            http.get(format!("{base}/me/feed"))
                .header("Authorization", &d.token)
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            401
        );
        let count = diesel::sql_query(
            "SELECT count(*)::bigint AS count FROM automation_audit_events WHERE api_client_id=$1",
        )
        .bind::<SqlUuid, _>(ctx.client.id)
        .get_result::<Count>(&mut pool.get().unwrap())
        .unwrap()
        .count;
        assert!(count >= 5);
        server.abort();
        // The fixture database itself is disposable; no production content is loaded.
    }
    #[test]
    fn delegation_is_deny_by_default() {
        let id = Uuid::new_v4();
        assert_eq!(
            required_scope("PATCH", &format!("/traces/{id}")),
            Some("content:write")
        );
        for path in [
            "/admin/users",
            "/sessions",
            "/users/google",
            "/users/apple",
            "/devices",
            "/user_secure_actions",
            "/journals/new_route",
            "/traces/../users",
        ] {
            assert!(required_scope("POST", path).is_none());
        }
        assert!(required_scope("POST", &format!("/journals/{id}/share_links")).is_none());
    }
}
