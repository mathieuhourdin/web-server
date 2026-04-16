use argon2::Config;
use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    error::{ErrorType, PpdcError},
    journal::{Journal, JournalStatus},
    post::Post,
    session::Session,
    user::User,
};
use crate::environment;
use crate::pagination::{PaginatedResponse, PaginationParams};
use crate::schema::journal_share_links;

const JOURNAL_SHARE_LINK_SECRET_BYTES: usize = 32;
const JOURNAL_SHARE_LINK_MAX_TTL_HOURS: i64 = 48;

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = crate::schema::journal_share_links)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct JournalShareLink {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub owner_user_id: Uuid,
    pub token_hash: String,
    pub expires_at: NaiveDateTime,
    pub revoked_at: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::journal_share_links)]
struct NewJournalShareLink {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub owner_user_id: Uuid,
    pub token_hash: String,
    pub expires_at: NaiveDateTime,
    pub revoked_at: Option<NaiveDateTime>,
}

#[derive(Deserialize)]
pub struct CreateJournalShareLinkDto {
    pub expires_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct JournalShareLinkCreationResponse {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub expires_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub url: String,
}

#[derive(Serialize)]
pub struct JournalShareLinkRevocationResponse {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub revoked_at: NaiveDateTime,
}

#[derive(Serialize)]
pub struct PublicSharedJournalShareLinkResponse {
    pub id: Uuid,
    pub expires_at: NaiveDateTime,
}

#[derive(Serialize)]
pub struct PublicSharedJournalOwnerResponse {
    pub id: Uuid,
    pub display_name: String,
    pub profile_picture_url: Option<String>,
}

#[derive(Serialize)]
pub struct PublicSharedJournalResponse {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub journal_type: crate::entities_v2::journal::JournalType,
    pub owner: PublicSharedJournalOwnerResponse,
}

#[derive(Serialize)]
pub struct PublicSharedJournalPostResponse {
    pub id: Uuid,
    pub title: String,
    pub subtitle: String,
    pub content: String,
    pub image_asset_id: Option<Uuid>,
    pub publishing_date: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Serialize)]
pub struct PublicSharedJournalBootstrapResponse {
    pub share_link: PublicSharedJournalShareLinkResponse,
    pub journal: PublicSharedJournalResponse,
    pub posts: PaginatedResponse<PublicSharedJournalPostResponse>,
}

#[derive(Deserialize)]
pub struct PublicShareLinkQuery {
    pub token: String,
    #[serde(flatten)]
    pub pagination: PaginationParams,
}

impl From<Post> for PublicSharedJournalPostResponse {
    fn from(post: Post) -> Self {
        Self {
            id: post.id,
            title: post.title,
            subtitle: post.subtitle,
            content: post.content,
            image_asset_id: post.image_asset_id,
            publishing_date: post.publishing_date,
            created_at: post.created_at,
            updated_at: post.updated_at,
        }
    }
}

impl JournalShareLink {
    fn is_accessible(&self) -> bool {
        self.revoked_at.is_none() && self.expires_at > Utc::now().naive_utc()
    }

    fn generate_secret() -> String {
        let bytes: [u8; JOURNAL_SHARE_LINK_SECRET_BYTES] = rand::thread_rng().gen();
        bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
    }

    fn hash_secret(secret: &str) -> Result<String, PpdcError> {
        let salt: [u8; 32] = rand::thread_rng().gen();
        let config = Config::default();
        argon2::hash_encoded(secret.as_bytes(), &salt, &config).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to hash journal share link secret: {:#?}", err),
            )
        })
    }

    fn verify_secret(secret_hash: &str, candidate: &str) -> Result<bool, PpdcError> {
        argon2::verify_encoded(secret_hash, candidate.as_bytes()).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to verify journal share link secret: {:#?}", err),
            )
        })
    }

    fn validate_requested_expiry(expires_at: DateTime<Utc>) -> Result<NaiveDateTime, PpdcError> {
        let now = Utc::now();
        if expires_at <= now {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "expires_at must be in the future".to_string(),
            ));
        }
        if expires_at > now + Duration::hours(JOURNAL_SHARE_LINK_MAX_TTL_HOURS) {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                format!(
                    "expires_at cannot be more than {} hours in the future",
                    JOURNAL_SHARE_LINK_MAX_TTL_HOURS
                ),
            ));
        }
        Ok(expires_at.naive_utc())
    }

    pub fn find(id: Uuid, pool: &DbPool) -> Result<Self, PpdcError> {
        let mut conn = pool.get()?;
        let link = journal_share_links::table
            .select(Self::as_select())
            .filter(journal_share_links::id.eq(id))
            .first::<Self>(&mut conn)
            .optional()?;

        link.ok_or_else(|| {
            PpdcError::new(404, ErrorType::ApiError, "Journal share link not found".to_string())
        })
    }

    pub fn create(
        journal: &Journal,
        owner_user_id: Uuid,
        expires_at: DateTime<Utc>,
        pool: &DbPool,
    ) -> Result<JournalShareLinkCreationResponse, PpdcError> {
        if journal.user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }
        if journal.is_encrypted {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot create a share link for an encrypted journal".to_string(),
            ));
        }
        if journal.status == JournalStatus::Archived {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot create a share link for an archived journal".to_string(),
            ));
        }

        let expires_at = Self::validate_requested_expiry(expires_at)?;
        let secret = Self::generate_secret();
        let token_hash = Self::hash_secret(&secret)?;

        let mut conn = pool.get()?;
        let link = diesel::insert_into(journal_share_links::table)
            .values(&NewJournalShareLink {
                id: Uuid::new_v4(),
                journal_id: journal.id,
                owner_user_id,
                token_hash,
                expires_at,
                revoked_at: None,
            })
            .returning(Self::as_returning())
            .get_result::<Self>(&mut conn)?;

        let url = format!(
            "{}/shared/journals/{}?token={}",
            environment::get_app_base_url().trim_end_matches('/'),
            link.id,
            secret
        );

        Ok(JournalShareLinkCreationResponse {
            id: link.id,
            journal_id: link.journal_id,
            expires_at: link.expires_at,
            created_at: link.created_at,
            url,
        })
    }

    pub fn revoke(self, owner_user_id: Uuid, pool: &DbPool) -> Result<JournalShareLinkRevocationResponse, PpdcError> {
        if self.owner_user_id != owner_user_id {
            return Err(PpdcError::unauthorized());
        }

        let revoked_at = Utc::now().naive_utc();
        let mut conn = pool.get()?;
        diesel::update(journal_share_links::table.filter(journal_share_links::id.eq(self.id)))
            .set((
                journal_share_links::revoked_at.eq(Some(revoked_at)),
                journal_share_links::updated_at.eq(revoked_at),
            ))
            .execute(&mut conn)?;

        Ok(JournalShareLinkRevocationResponse {
            id: self.id,
            journal_id: self.journal_id,
            revoked_at,
        })
    }

    fn authorize(
        id: Uuid,
        token: &str,
        pool: &DbPool,
    ) -> Result<(JournalShareLink, Journal), PpdcError> {
        let link = Self::find(id, pool)?;
        if !Self::verify_secret(&link.token_hash, token)? {
            return Err(PpdcError::unauthorized());
        }
        if !link.is_accessible() {
            return Err(PpdcError::new(
                410,
                ErrorType::ApiError,
                "Journal share link is expired or revoked".to_string(),
            ));
        }

        let journal = Journal::find_full(link.journal_id, pool)?;
        if journal.is_encrypted || journal.status == JournalStatus::Archived {
            return Err(PpdcError::new(
                410,
                ErrorType::ApiError,
                "Shared journal is no longer available".to_string(),
            ));
        }

        Ok((link, journal))
    }
}

#[debug_handler]
pub async fn post_journal_share_link_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(journal_id): Path<Uuid>,
    Json(payload): Json<CreateJournalShareLinkDto>,
) -> Result<Json<JournalShareLinkCreationResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let journal = Journal::find_full(journal_id, &pool)?;
    let response = JournalShareLink::create(&journal, user_id, payload.expires_at, &pool)?;
    Ok(Json(response))
}

#[debug_handler]
pub async fn delete_journal_share_link_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((journal_id, share_link_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<JournalShareLinkRevocationResponse>, PpdcError> {
    let user_id = session.user_id.ok_or_else(PpdcError::unauthorized)?;
    let link = JournalShareLink::find(share_link_id, &pool)?;
    if link.journal_id != journal_id {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Journal share link not found for this journal".to_string(),
        ));
    }
    let response = link.revoke(user_id, &pool)?;
    Ok(Json(response))
}

fn build_public_journal_response(journal: Journal, owner: User) -> PublicSharedJournalResponse {
    PublicSharedJournalResponse {
        id: journal.id,
        title: journal.title,
        subtitle: journal.subtitle,
        journal_type: journal.journal_type,
        owner: PublicSharedJournalOwnerResponse {
            id: owner.id,
            display_name: owner.display_name(),
            profile_picture_url: owner.profile_picture_url,
        },
    }
}

#[debug_handler]
pub async fn get_shared_journal_route(
    Extension(pool): Extension<DbPool>,
    Path(share_link_id): Path<Uuid>,
    Query(params): Query<PublicShareLinkQuery>,
) -> Result<Json<PublicSharedJournalBootstrapResponse>, PpdcError> {
    let pagination = params.pagination.validate()?;
    let (link, journal) = JournalShareLink::authorize(share_link_id, &params.token, &pool)?;
    let owner = User::find(&journal.user_id, &pool)?;
    let (posts, total) = Post::find_public_default_for_journal_paginated(
        journal.id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;

    Ok(Json(PublicSharedJournalBootstrapResponse {
        share_link: PublicSharedJournalShareLinkResponse {
            id: link.id,
            expires_at: link.expires_at,
        },
        journal: build_public_journal_response(journal, owner),
        posts: PaginatedResponse::new(
            posts
                .into_iter()
                .map(PublicSharedJournalPostResponse::from)
                .collect(),
            pagination,
            total,
        ),
    }))
}

#[debug_handler]
pub async fn get_shared_journal_posts_route(
    Extension(pool): Extension<DbPool>,
    Path(share_link_id): Path<Uuid>,
    Query(params): Query<PublicShareLinkQuery>,
) -> Result<Json<PaginatedResponse<PublicSharedJournalPostResponse>>, PpdcError> {
    let pagination = params.pagination.validate()?;
    let (_, journal) = JournalShareLink::authorize(share_link_id, &params.token, &pool)?;
    let (posts, total) = Post::find_public_default_for_journal_paginated(
        journal.id,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;

    Ok(Json(PaginatedResponse::new(
        posts
            .into_iter()
            .map(PublicSharedJournalPostResponse::from)
            .collect(),
        pagination,
        total,
    )))
}
