use crate::{
    db::DbPool,
    entities_v2::{
        asset::{Asset, AssetStatus},
        error::{ErrorType, PpdcError},
    },
    pagination::{PaginatedResponse, ValidatedPagination},
};
use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn error(status: u32, message: &str) -> PpdcError {
    PpdcError::new(status, ErrorType::ApiError, message.to_owned())
}

#[derive(Debug, Serialize, QueryableByName)]
pub struct Blog {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[serde(skip_serializing)]
    #[diesel(sql_type=SqlUuid)]
    pub user_id: Uuid,
    #[diesel(sql_type=Text)]
    pub slug: String,
    #[diesel(sql_type=Text)]
    pub name: String,
    #[diesel(sql_type=Text)]
    pub username: String,
    #[diesel(sql_type=Text)]
    pub description: String,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub image_asset_id: Option<Uuid>,
    #[diesel(sql_type=Bool)]
    pub enabled: bool,
    #[diesel(sql_type=Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type=Timestamp)]
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlogInput {
    pub slug: String,
    pub name: String,
    pub username: String,
    #[serde(default)]
    pub description: String,
    pub image_asset_id: Option<Uuid>,
    #[serde(default)]
    pub enabled: bool,
}

const BLOG_SELECT: &str = "SELECT c.id,c.user_id,c.enabled,c.created_at,c.updated_at,
    b.slug,b.name,b.username,b.description,b.image_asset_id
    FROM blogs b JOIN publication_channels c ON c.id=b.channel_id";

#[derive(Serialize, QueryableByName)]
pub struct BlogPost {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=SqlUuid)]
    pub publication_channel_id: Uuid,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub source_trace_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub source_album_id: Option<Uuid>,
    #[diesel(sql_type=Text)]
    pub status: String,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub publishing_date: Option<NaiveDateTime>,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub last_publication_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub last_publication_at: Option<NaiveDateTime>,
    #[diesel(sql_type=Text)]
    pub title: String,
    #[diesel(sql_type=Text)]
    pub subtitle: String,
    #[diesel(sql_type=Text)]
    pub content: String,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    pub image_asset_id: Option<Uuid>,
    #[diesel(sql_type=Nullable<Timestamp>)]
    pub finalized_at: Option<NaiveDateTime>,
}

// A deliberately narrow public projection. No journal/mention/translation/source
// asset metadata is hydrated, and public publication never grants source access.
const POST_FROM: &str = " FROM posts p
    LEFT JOIN traces t ON t.id=p.source_trace_id
    LEFT JOIN albums a ON a.id=p.source_album_id";
const POST_COLUMNS: &str =
    "SELECT p.id,p.publication_channel_id,p.source_trace_id,p.source_album_id,
    p.status,p.publishing_date,p.last_publication_id,p.last_publication_at,COALESCE(t.title,a.title,'') AS title,
    COALESCE(t.subtitle,a.subtitle,'') AS subtitle,COALESCE(t.content,a.content,'') AS content,
    COALESCE(t.content_image_asset_id,a.cover_image_asset_id) AS image_asset_id,t.finalized_at";
// Check the current source on every anonymous read, not just at publication.
const PUBLIC_SOURCE: &str = " AND p.status='PUBLISHED' AND (
    (t.user_id=p.user_id AND t.status='FINALIZED' AND NOT t.is_encrypted AND t.trace_type='USER_TRACE')
    OR (a.owner_user_id=p.user_id AND a.completion_status <> 'ARCHIVED'))";

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    count: i64,
}

impl BlogInput {
    pub fn validate(&self) -> Result<(), PpdcError> {
        let reserved = [
            "api", "blog", "blogs", "public", "assets", "admin", "robots", "sitemap",
        ];
        if self.slug.len() > 80
            || !regex::Regex::new(r"^[a-z0-9]+(-[a-z0-9]+)*$")
                .unwrap()
                .is_match(&self.slug)
            || reserved.contains(&self.slug.as_str())
        {
            return Err(error(400,"slug must be lowercase URL-safe words separated by hyphens (1–80 characters), and not reserved"));
        }
        if self.name.trim().is_empty()
            || self.name.chars().count() > 160
            || self.username.trim().is_empty()
            || self.username.chars().count() > 120
            || self.description.chars().count() > 4000
        {
            return Err(error(400,"Provide a blog name (1–160 characters), author username (1–120) and description (at most 4000)"));
        }
        Ok(())
    }
}

pub fn validate_image(id: Option<Uuid>, owner: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    if let Some(id) = id {
        let a = Asset::find(id, pool)?;
        if a.owner_user_id != owner || a.status != AssetStatus::Ready || !safe_image(&a.mime_type) {
            return Err(error(
                400,
                "Blog image must be an owned, ready JPEG, PNG, WebP or GIF asset",
            ));
        }
    }
    Ok(())
}
pub fn safe_image(mime: &str) -> bool {
    matches!(
        mime,
        "image/jpeg" | "image/png" | "image/webp" | "image/gif"
    )
}

pub fn owned(id: Uuid, owner: Uuid, pool: &DbPool) -> Result<Blog, PpdcError> {
    Ok(
        diesel::sql_query(format!("{BLOG_SELECT} WHERE c.id=$1 AND c.user_id=$2"))
            .bind::<SqlUuid, _>(id)
            .bind::<SqlUuid, _>(owner)
            .get_result(&mut pool.get()?)?,
    )
}
pub fn public(slug: &str, pool: &DbPool) -> Result<Blog, PpdcError> {
    Ok(diesel::sql_query(format!(
        "{BLOG_SELECT} WHERE b.slug=$1 AND c.enabled
        AND NOT EXISTS(SELECT 1 FROM automation_demo_users d WHERE d.user_id=c.user_id)"
    ))
    .bind::<Text, _>(slug)
    .get_result(&mut pool.get()?)?)
}
pub fn list(
    owner: Uuid,
    page: ValidatedPagination,
    pool: &DbPool,
) -> Result<PaginatedResponse<Blog>, PpdcError> {
    let mut conn = pool.get()?;
    let total=diesel::sql_query("SELECT COUNT(*) FROM blogs b JOIN publication_channels c ON c.id=b.channel_id WHERE c.user_id=$1")
        .bind::<SqlUuid,_>(owner).get_result::<Count>(&mut conn)?.count;
    let items = diesel::sql_query(format!(
        "{BLOG_SELECT} WHERE c.user_id=$1 ORDER BY c.created_at DESC,c.id DESC LIMIT $2 OFFSET $3"
    ))
    .bind::<SqlUuid, _>(owner)
    .bind::<BigInt, _>(page.limit)
    .bind::<BigInt, _>(page.offset)
    .load(&mut conn)?;
    Ok(PaginatedResponse::new(items, page, total))
}
pub fn save(
    id: Option<Uuid>,
    owner: Uuid,
    input: BlogInput,
    pool: &DbPool,
) -> Result<Blog, PpdcError> {
    input.validate()?;
    if crate::entities_v2::platform_infra::automation::demo_owner(owner, pool)?.is_some() {
        return Err(error(403, "Demo accounts cannot create public blogs"));
    }
    validate_image(input.image_asset_id, owner, pool)?;
    if let Some(id) = id {
        owned(id, owner, pool)?;
    }
    let channel = id.unwrap_or_else(Uuid::new_v4);
    pool.get()?.transaction::<_,PpdcError,_>(|conn|{
        if id.is_none(){
            diesel::sql_query("INSERT INTO publication_channels(id,user_id,kind,enabled) VALUES($1,$2,'BLOG',$3)")
                .bind::<SqlUuid,_>(channel).bind::<SqlUuid,_>(owner).bind::<Bool,_>(input.enabled).execute(conn)?;
        } else {
            diesel::sql_query("UPDATE publication_channels SET enabled=$3,updated_at=NOW() WHERE id=$1 AND user_id=$2")
                .bind::<SqlUuid,_>(channel).bind::<SqlUuid,_>(owner).bind::<Bool,_>(input.enabled).execute(conn)?;
        }
        diesel::sql_query("INSERT INTO blogs(channel_id,slug,name,username,description,image_asset_id) VALUES($1,$2,$3,$4,$5,$6)
            ON CONFLICT(channel_id) DO UPDATE SET slug=EXCLUDED.slug,name=EXCLUDED.name,username=EXCLUDED.username,
                description=EXCLUDED.description,image_asset_id=EXCLUDED.image_asset_id")
            .bind::<SqlUuid,_>(channel).bind::<Text,_>(&input.slug).bind::<Text,_>(input.name.trim())
            .bind::<Text,_>(input.username.trim()).bind::<Text,_>(&input.description)
            .bind::<Nullable<SqlUuid>,_>(input.image_asset_id).execute(conn)?;
        Ok(())
    })?;
    owned(channel, owner, pool)
}

pub fn posts(
    blog: &Blog,
    anonymous: bool,
    page: ValidatedPagination,
    pool: &DbPool,
) -> Result<PaginatedResponse<BlogPost>, PpdcError> {
    let predicate = if anonymous { PUBLIC_SOURCE } else { "" };
    let mut conn = pool.get()?;
    let total = diesel::sql_query(format!(
        "SELECT COUNT(*){POST_FROM} WHERE p.publication_channel_id=$1{predicate}"
    ))
    .bind::<SqlUuid, _>(blog.id)
    .get_result::<Count>(&mut conn)?
    .count;
    let items = diesel::sql_query(format!(
        "{POST_COLUMNS}{POST_FROM} WHERE p.publication_channel_id=$1{predicate}
        ORDER BY p.last_publication_at DESC NULLS LAST,p.id DESC LIMIT $2 OFFSET $3"
    ))
    .bind::<SqlUuid, _>(blog.id)
    .bind::<BigInt, _>(page.limit)
    .bind::<BigInt, _>(page.offset)
    .load(&mut conn)?;
    Ok(PaginatedResponse::new(items, page, total))
}
pub fn post(blog: &Blog, id: Uuid, anonymous: bool, pool: &DbPool) -> Result<BlogPost, PpdcError> {
    let predicate = if anonymous { PUBLIC_SOURCE } else { "" };
    Ok(diesel::sql_query(format!(
        "{POST_COLUMNS}{POST_FROM} WHERE p.publication_channel_id=$1 AND p.id=$2{predicate}"
    ))
    .bind::<SqlUuid, _>(blog.id)
    .bind::<SqlUuid, _>(id)
    .get_result(&mut pool.get()?)?)
}

pub fn publish(
    blog: &Blog,
    source: Uuid,
    album: bool,
    pool: &DbPool,
) -> Result<BlogPost, PpdcError> {
    // Source locks serialize publication with archive/encryption and ownership changes.
    let id=pool.get()?.transaction::<_,PpdcError,_>(|conn|{
        #[derive(QueryableByName)] struct Id { #[diesel(sql_type=SqlUuid)] id:Uuid }
        let query=if album {
            "SELECT id FROM albums WHERE id=$1 AND owner_user_id=$2 AND completion_status<>'ARCHIVED' FOR SHARE"
        } else {
            "SELECT id FROM traces WHERE id=$1 AND user_id=$2 AND status='FINALIZED' AND NOT is_encrypted AND trace_type='USER_TRACE' FOR SHARE"
        };
        diesel::sql_query(query).bind::<SqlUuid,_>(source).bind::<SqlUuid,_>(blog.user_id)
            .get_result::<Id>(conn).optional()?.ok_or_else(||error(400,"Source must be your own publishable album or finalized, non-encrypted ordinary trace"))?;
        let column=if album {"source_album_id"}else{"source_trace_id"};
        let row=diesel::sql_query(format!("INSERT INTO posts(id,user_id,publication_channel_id,{column},title,subtitle,content,
                interaction_type,post_type,status,publishing_date,notification_mode)
            VALUES($1,$2,$3,$4,'','','','OUTPUT','IDEA','PUBLISHED',NOW(),'NONE')
            ON CONFLICT(publication_channel_id,{column}) WHERE publication_channel_id IS NOT NULL AND {column} IS NOT NULL
            DO UPDATE SET status='PUBLISHED',publishing_date=COALESCE(posts.publishing_date,NOW()),updated_at=NOW() RETURNING id"))
            .bind::<SqlUuid,_>(Uuid::new_v4()).bind::<SqlUuid,_>(blog.user_id)
            .bind::<SqlUuid,_>(blog.id).bind::<SqlUuid,_>(source).get_result::<Id>(conn)?;
        Ok(row.id)
    })?;
    post(blog, id, false, pool)
}
pub fn unpublish(blog: &Blog, id: Uuid, pool: &DbPool) -> Result<(), PpdcError> {
    let n=diesel::sql_query("UPDATE posts SET status='ARCHIVED',updated_at=NOW() WHERE id=$1 AND publication_channel_id=$2")
        .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(blog.id).execute(&mut pool.get()?)?;
    if n == 0 {
        return Err(error(404, "Blog post not found"));
    }
    Ok(())
}

pub fn delete(blog: &Blog, pool: &DbPool) -> Result<(), PpdcError> {
    diesel::sql_query("DELETE FROM publication_channels WHERE id=$1 AND user_id=$2")
        .bind::<SqlUuid, _>(blog.id)
        .bind::<SqlUuid, _>(blog.user_id)
        .execute(&mut pool.get()?)?;
    Ok(())
}

// Subject membership is live, but is never a publication/access grant.
pub fn subject_items(
    blog: &Blog,
    subject: Uuid,
    page: ValidatedPagination,
    pool: &DbPool,
) -> Result<PaginatedResponse<BlogPost>, PpdcError> {
    let parent = post(blog, subject, true, pool)?;
    let album = parent
        .source_album_id
        .ok_or_else(|| error(404, "Subject not found"))?;
    let predicate=format!("{POST_FROM} JOIN album_items i ON i.trace_id=t.id JOIN albums subject ON subject.id=i.album_id
        WHERE p.publication_channel_id=$1 AND i.album_id=$2 {PUBLIC_SOURCE}");
    let mut conn = pool.get()?;
    let total = diesel::sql_query(format!("SELECT COUNT(*){predicate}"))
        .bind::<SqlUuid, _>(blog.id)
        .bind::<SqlUuid, _>(album)
        .get_result::<Count>(&mut conn)?
        .count;
    let items=diesel::sql_query(format!("{POST_COLUMNS}{predicate}
        ORDER BY CASE WHEN subject.ordering_mode='MANUAL' THEN i.ordering_index END ASC,
            CASE WHEN subject.ordering_mode='ADDED_AT' THEN i.created_at ELSE t.finalized_at END ASC,p.id ASC LIMIT $3 OFFSET $4"))
        .bind::<SqlUuid,_>(blog.id).bind::<SqlUuid,_>(album).bind::<BigInt,_>(page.limit).bind::<BigInt,_>(page.offset).load(&mut conn)?;
    Ok(PaginatedResponse::new(items, page, total))
}
