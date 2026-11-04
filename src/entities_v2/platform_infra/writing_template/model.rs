use crate::{
    entities_v2::error::{ErrorType, PpdcError},
    pagination::{PaginatedResponse, PaginationParams},
};
use chrono::NaiveDateTime;
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Nullable, Text, Timestamp, Uuid as SqlUuid},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, QueryableByName)]
pub struct WritingTemplate {
    #[diesel(sql_type=SqlUuid)]
    pub id: Uuid,
    #[diesel(sql_type=Text)]
    pub slug: String,
    #[diesel(sql_type=Text)]
    pub title: String,
    #[diesel(sql_type=Text)]
    pub description: String,
    #[diesel(sql_type=Text)]
    pub content: String,
    #[diesel(sql_type=Text)]
    pub locale: String,
    #[diesel(sql_type=diesel::sql_types::Integer)]
    pub sort_order: i32,
    #[diesel(sql_type=Bool)]
    pub is_active: bool,
    #[diesel(sql_type=Timestamp)]
    pub created_at: NaiveDateTime,
    #[diesel(sql_type=Timestamp)]
    pub updated_at: NaiveDateTime,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewTemplate {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub content: String,
    #[serde(default = "default_locale")]
    pub locale: String,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default = "active")]
    pub is_active: bool,
}
fn default_locale() -> String {
    "fr".into()
}
fn active() -> bool {
    true
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatchTemplate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    pub locale: Option<String>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(flatten)]
    pub pagination: PaginationParams,
    pub locale: Option<String>,
    pub is_active: Option<bool>,
}

fn invalid(message: &str) -> PpdcError {
    PpdcError::new(400, ErrorType::ApiError, message.into())
}
fn not_found() -> PpdcError {
    PpdcError::new(
        404,
        ErrorType::ApiError,
        "Writing template not found".into(),
    )
}

pub(super) fn normalize_locale(value: &str) -> Result<String, PpdcError> {
    let value = value.trim().to_ascii_lowercase();
    let mut parts = value.split('-');
    let language = parts.next().unwrap_or("");
    if value.len() > 35
        || !(2..=3).contains(&language.len())
        || !language.bytes().all(|b| b.is_ascii_lowercase())
        || !parts
            .all(|p| (2..=8).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return Err(invalid(
            "locale must be a language tag such as fr, en or fr-CA",
        ));
    }
    Ok(value)
}

impl NewTemplate {
    pub(super) fn validate(&mut self) -> Result<(), PpdcError> {
        self.slug = self.slug.trim().to_string();
        if self.slug.is_empty()
            || self.slug.len() > 80
            || !self.slug.as_bytes()[0].is_ascii_lowercase()
            || !self
                .slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        {
            return Err(invalid("slug must start with a lowercase letter and contain at most 80 lowercase letters, digits, underscores or hyphens"));
        }
        self.title = self.title.trim().to_string();
        self.description = self.description.trim().to_string();
        self.locale = normalize_locale(&self.locale)?;
        if self.title.is_empty() || self.title.chars().count() > 200 {
            return Err(invalid("title must contain between 1 and 200 characters"));
        }
        if self.description.chars().count() > 2000 {
            return Err(invalid("description must not exceed 2000 characters"));
        }
        if self.content.trim().is_empty() || self.content.chars().count() > 20_000 {
            return Err(invalid(
                "content must contain between 1 and 20000 characters",
            ));
        }
        Ok(())
    }

    pub fn create(mut self, conn: &mut PgConnection) -> Result<WritingTemplate, PpdcError> {
        self.validate()?;
        Ok(diesel::sql_query("INSERT INTO writing_templates(slug,title,description,content,locale,sort_order,is_active)
            VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING *")
            .bind::<Text,_>(self.slug).bind::<Text,_>(self.title).bind::<Text,_>(self.description)
            .bind::<Text,_>(self.content).bind::<Text,_>(self.locale)
            .bind::<diesel::sql_types::Integer,_>(self.sort_order).bind::<Bool,_>(self.is_active)
            .get_result(conn)?)
    }
}

impl WritingTemplate {
    pub fn find(id: Uuid, conn: &mut PgConnection) -> Result<Self, PpdcError> {
        diesel::sql_query("SELECT * FROM writing_templates WHERE id=$1")
            .bind::<SqlUuid, _>(id)
            .get_result(conn)
            .optional()?
            .ok_or_else(not_found)
    }

    pub fn list(
        query: ListQuery,
        admin: bool,
        conn: &mut PgConnection,
    ) -> Result<PaginatedResponse<Self>, PpdcError> {
        let page = query.pagination.validate()?;
        let locale = query.locale.as_deref().map(normalize_locale).transpose()?;
        if !admin && query.is_active == Some(false) {
            return Err(invalid("Only administrators can list inactive templates"));
        }
        let is_active = if admin { query.is_active } else { Some(true) };
        #[derive(QueryableByName)]
        struct Count {
            #[diesel(sql_type=BigInt)]
            count: i64,
        }
        let total = diesel::sql_query(
            "SELECT COUNT(*)::bigint AS count FROM writing_templates
            WHERE ($1::text IS NULL OR locale=$1) AND ($2::boolean IS NULL OR is_active=$2)",
        )
        .bind::<Nullable<Text>, _>(&locale)
        .bind::<Nullable<Bool>, _>(is_active)
        .get_result::<Count>(conn)?
        .count;
        let items = diesel::sql_query(
            "SELECT * FROM writing_templates
            WHERE ($1::text IS NULL OR locale=$1) AND ($2::boolean IS NULL OR is_active=$2)
            ORDER BY sort_order,slug,id LIMIT $3 OFFSET $4",
        )
        .bind::<Nullable<Text>, _>(locale)
        .bind::<Nullable<Bool>, _>(is_active)
        .bind::<BigInt, _>(page.limit)
        .bind::<BigInt, _>(page.offset)
        .load(conn)?;
        Ok(PaginatedResponse {
            items,
            total,
            limit: page.limit,
            offset: page.offset,
        })
    }

    pub fn patch(
        id: Uuid,
        patch: PatchTemplate,
        conn: &mut PgConnection,
    ) -> Result<Self, PpdcError> {
        conn.transaction(|conn| {
            let old: Self =
                diesel::sql_query("SELECT * FROM writing_templates WHERE id=$1 FOR UPDATE")
                    .bind::<SqlUuid, _>(id)
                    .get_result(conn)
                    .optional()?
                    .ok_or_else(not_found)?;
            let mut updated = NewTemplate {
                slug: old.slug,
                title: patch.title.unwrap_or(old.title),
                description: patch.description.unwrap_or(old.description),
                content: patch.content.unwrap_or(old.content),
                locale: patch.locale.unwrap_or(old.locale),
                sort_order: patch.sort_order.unwrap_or(old.sort_order),
                is_active: patch.is_active.unwrap_or(old.is_active),
            };
            updated.validate()?;
            Ok(diesel::sql_query(
                "UPDATE writing_templates SET title=$2,description=$3,content=$4,
                locale=$5,sort_order=$6,is_active=$7,updated_at=NOW() WHERE id=$1 RETURNING *",
            )
            .bind::<SqlUuid, _>(id)
            .bind::<Text, _>(updated.title)
            .bind::<Text, _>(updated.description)
            .bind::<Text, _>(updated.content)
            .bind::<Text, _>(updated.locale)
            .bind::<diesel::sql_types::Integer, _>(updated.sort_order)
            .bind::<Bool, _>(updated.is_active)
            .get_result(conn)?)
        })
    }
}

/// Validate provenance in the same transaction as trace content/version changes.
/// Existing associations survive deactivation; new choices must be active.
pub(crate) fn validate_trace_template(
    trace_id: Option<Uuid>,
    choice: Option<Uuid>,
    trace_type: &str,
    conn: &mut PgConnection,
) -> Result<(), PpdcError> {
    if let Some(id) = trace_id {
        #[derive(QueryableByName)]
        struct Previous {
            #[diesel(sql_type=Nullable<SqlUuid>)]
            writing_template_id: Option<Uuid>,
            #[diesel(sql_type=Text)]
            status: String,
        }
        let previous = diesel::sql_query(
            "SELECT writing_template_id,status FROM traces WHERE id=$1 FOR UPDATE",
        )
        .bind::<SqlUuid, _>(id)
        .get_result::<Previous>(conn)?;
        if previous.writing_template_id == choice {
            return Ok(());
        }
        if previous.status != "DRAFT" {
            return Err(invalid(
                "Writing template associations can only be changed on drafts",
            ));
        }
    } else if choice.is_none() {
        return Ok(());
    }
    if !matches!(trace_type, "USER_TRACE" | "WORKSPACE_TRACE") {
        return Err(invalid(
            "Writing templates are only supported on ordinary writing traces",
        ));
    }
    if let Some(id) = choice {
        // FOR SHARE also serializes with an administrator deactivating it.
        #[derive(QueryableByName)]
        struct Id {
            #[diesel(sql_type=SqlUuid)]
            _id: Uuid,
        }
        let found = diesel::sql_query(
            "SELECT id AS _id FROM writing_templates WHERE id=$1 AND is_active=TRUE FOR SHARE",
        )
        .bind::<SqlUuid, _>(id)
        .get_result::<Id>(conn)
        .optional()?;
        if found.is_none() {
            return Err(invalid("Writing template does not exist or is inactive"));
        }
    }
    Ok(())
}
