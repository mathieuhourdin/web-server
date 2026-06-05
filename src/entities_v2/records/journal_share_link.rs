use axum::{
    debug_handler,
    extract::{Extension, Json, Path, Query},
    response::Redirect,
};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::db::DbPool;
use crate::entities_v2::{
    asset::Asset,
    error::{ErrorType, PpdcError},
    journal::{Journal, JournalStatus},
    post::{Post, PostAudienceRole, PostStatus},
    session::Session,
    trace::{Trace, TraceSharingSensitivity},
    user::User,
};
use crate::environment;
use crate::pagination::{PaginatedResponse, PaginationParams};
use crate::schema::journal_share_links;

const JOURNAL_SHARE_LINK_SECRET_BYTES: usize = 32;
const JOURNAL_SHARE_LINK_MAX_TTL_HOURS: i64 = 48;
const JOURNAL_SHARE_LINK_HMAC_PREFIX: &str = "hmac_sha256:";

type HmacSha256 = Hmac<Sha256>;

enum SecretVerification {
    Invalid,
    Verified,
    VerifiedAndUpgrade(String),
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = crate::schema::journal_share_links)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct JournalShareLink {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub owner_user_id: Uuid,
    pub token_hash: String,
    pub scoped_post_id: Option<Uuid>,
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
    pub scoped_post_id: Option<Uuid>,
    pub expires_at: NaiveDateTime,
    pub revoked_at: Option<NaiveDateTime>,
}

#[derive(Deserialize)]
pub struct CreateJournalShareLinkDto {
    pub expires_at: DateTime<Utc>,
    pub scoped_post_id: Option<Uuid>,
}

#[derive(Serialize)]
pub struct JournalShareLinkCreationResponse {
    pub id: Uuid,
    pub journal_id: Uuid,
    pub scoped_post_id: Option<Uuid>,
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
    pub scoped_post_id: Option<Uuid>,
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
    pub content: String,
    pub image_url: Option<String>,
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

#[derive(Deserialize)]
pub struct PublicShareLinkAssetQuery {
    pub token: String,
}

fn build_shared_journal_asset_url(share_link_id: Uuid, asset_id: Uuid, token: &str) -> String {
    format!(
        "{}/shared/journals/{}/assets/{}?token={}",
        environment::get_api_url().trim_end_matches('/'),
        share_link_id,
        asset_id,
        token
    )
}

impl PublicSharedJournalPostResponse {
    fn from_post(post: Post, share_link_id: Uuid, token: &str, pool: &DbPool) -> Result<Self, PpdcError> {
        let image_asset_id = match post.source_trace_id {
            Some(trace_id) => Trace::find_full_trace(trace_id, pool)?.content_image_asset_id,
            None => None,
        };
        let image_url = image_asset_id.map(|asset_id| {
            build_shared_journal_asset_url(share_link_id, asset_id, token)
        });

        Ok(Self {
            id: post.id,
            title: post.title,
            content: post.content,
            image_url,
            image_asset_id,
            publishing_date: post.publishing_date,
            created_at: post.created_at,
            updated_at: post.updated_at,
        })
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
        let hmac_secret = environment::get_journal_share_link_hmac_secret();
        let mut mac = HmacSha256::new_from_slice(hmac_secret.as_bytes()).map_err(|err| {
            PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to initialize journal share link HMAC: {:#?}", err),
            )
        })?;
        mac.update(secret.as_bytes());
        let digest = mac.finalize().into_bytes();

        Ok(format!(
            "{}{}",
            JOURNAL_SHARE_LINK_HMAC_PREFIX,
            Self::hex_encode(&digest)
        ))
    }

    fn hex_encode(bytes: &[u8]) -> String {
        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            encoded.push_str(&format!("{:02x}", byte));
        }
        encoded
    }

    fn verify_secret(secret_hash: &str, candidate: &str) -> Result<SecretVerification, PpdcError> {
        if let Some(expected_digest) = secret_hash.strip_prefix(JOURNAL_SHARE_LINK_HMAC_PREFIX) {
            let candidate_digest = Self::hash_secret(candidate)?;
            let Some(candidate_digest) =
                candidate_digest.strip_prefix(JOURNAL_SHARE_LINK_HMAC_PREFIX)
            else {
                return Err(PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    "Invalid journal share link hash format".to_string(),
                ));
            };

            return Ok(
                if expected_digest
                    .as_bytes()
                    .ct_eq(candidate_digest.as_bytes())
                    .into()
                {
                    SecretVerification::Verified
                } else {
                    SecretVerification::Invalid
                },
            );
        }

        let verified =
            argon2::verify_encoded(secret_hash, candidate.as_bytes()).map_err(|err| {
                PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("Unable to verify journal share link secret: {:#?}", err),
                )
            })?;

        if verified {
            Ok(SecretVerification::VerifiedAndUpgrade(Self::hash_secret(
                candidate,
            )?))
        } else {
            Ok(SecretVerification::Invalid)
        }
    }

    fn persist_upgraded_secret_hash(
        link_id: Uuid,
        upgraded_secret_hash: &str,
        pool: &DbPool,
    ) -> Result<(), PpdcError> {
        let mut conn = pool.get()?;
        diesel::update(journal_share_links::table.filter(journal_share_links::id.eq(link_id)))
            .set((
                journal_share_links::token_hash.eq(upgraded_secret_hash),
                journal_share_links::updated_at.eq(Utc::now().naive_utc()),
            ))
            .execute(&mut conn)?;
        Ok(())
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
            PpdcError::new(
                404,
                ErrorType::ApiError,
                "Journal share link not found".to_string(),
            )
        })
    }

    pub fn create(
        journal: &Journal,
        owner_user_id: Uuid,
        expires_at: DateTime<Utc>,
        scoped_post_id: Option<Uuid>,
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
        if let Some(scoped_post_id) = scoped_post_id {
            Self::validate_scoped_post(journal, scoped_post_id, pool)?;
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
                scoped_post_id,
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
            scoped_post_id: link.scoped_post_id,
            expires_at: link.expires_at,
            created_at: link.created_at,
            url,
        })
    }

    pub fn revoke(
        self,
        owner_user_id: Uuid,
        pool: &DbPool,
    ) -> Result<JournalShareLinkRevocationResponse, PpdcError> {
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
        let mut link = Self::find(id, pool)?;
        match Self::verify_secret(&link.token_hash, token)? {
            SecretVerification::Invalid => return Err(PpdcError::unauthorized()),
            SecretVerification::Verified => {}
            SecretVerification::VerifiedAndUpgrade(upgraded_secret_hash) => {
                Self::persist_upgraded_secret_hash(link.id, &upgraded_secret_hash, pool)?;
                link.token_hash = upgraded_secret_hash;
            }
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

    fn validate_scoped_post(
        journal: &Journal,
        post_id: Uuid,
        pool: &DbPool,
    ) -> Result<Post, PpdcError> {
        let post = Post::find_full(post_id, pool)?;
        if post.user_id != journal.user_id {
            return Err(PpdcError::unauthorized());
        }
        if post.status != PostStatus::Published {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot scope a journal share link to a non-published post".to_string(),
            ));
        }
        if post.audience_role != PostAudienceRole::Default {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot scope a journal share link to a restricted post".to_string(),
            ));
        }
        let trace_id = post.source_trace_id.ok_or_else(|| {
            PpdcError::new(
                400,
                ErrorType::ApiError,
                "Scoped post must be linked to a trace".to_string(),
            )
        })?;
        let trace = Trace::find_full_trace(trace_id, pool)?;
        if trace.journal_id != Some(journal.id) {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Scoped post must belong to the shared journal".to_string(),
            ));
        }
        if trace.sharing_sensitivity != TraceSharingSensitivity::Normal {
            return Err(PpdcError::new(
                400,
                ErrorType::ApiError,
                "Cannot scope a journal share link to a sensitive trace".to_string(),
            ));
        }
        Ok(post)
    }

    fn find_public_posts_for_link(
        link: &JournalShareLink,
        journal: &Journal,
        offset: i64,
        limit: i64,
        pool: &DbPool,
    ) -> Result<(Vec<Post>, i64), PpdcError> {
        let Some(scoped_post_id) = link.scoped_post_id else {
            return Post::find_public_default_for_journal_paginated(
                journal.id, offset, limit, pool,
            );
        };

        let post = match Self::validate_scoped_post(journal, scoped_post_id, pool) {
            Ok(post) => post,
            Err(err) if err.status_code == 400 || err.status_code == 404 => {
                return Ok((vec![], 0));
            }
            Err(err) => return Err(err),
        };

        if offset > 0 || limit <= 0 {
            return Ok((vec![], 1));
        }
        Ok((vec![post], 1))
    }

    fn public_link_uses_image_asset(
        link: &JournalShareLink,
        journal: &Journal,
        asset_id: Uuid,
        pool: &DbPool,
    ) -> Result<bool, PpdcError> {
        if let Some(scoped_post_id) = link.scoped_post_id {
            let Ok(post) = Self::validate_scoped_post(journal, scoped_post_id, pool) else {
                return Ok(false);
            };
            let Some(trace_id) = post.source_trace_id else {
                return Ok(false);
            };
            return Ok(Trace::find_full_trace(trace_id, pool)?.content_image_asset_id == Some(asset_id));
        }

        Post::public_default_post_uses_image_asset_in_journal(journal.id, asset_id, pool)
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
    let response = JournalShareLink::create(
        &journal,
        user_id,
        payload.expires_at,
        payload.scoped_post_id,
        &pool,
    )?;
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

async fn authorize_shared_journal_asset_redirect(
    share_link_id: Uuid,
    asset_id: Uuid,
    token: &str,
    pool: &DbPool,
) -> Result<Redirect, PpdcError> {
    let (link, journal) = JournalShareLink::authorize(share_link_id, token, pool)?;

    if !JournalShareLink::public_link_uses_image_asset(&link, &journal, asset_id, pool)? {
        return Err(PpdcError::new(
            404,
            ErrorType::ApiError,
            "Shared journal asset not found".to_string(),
        ));
    }

    let asset = Asset::find(asset_id, pool)?;
    let (url, _) = asset
        .signed_read_url(crate::environment::get_assets_signed_url_ttl_seconds())
        .await?;

    Ok(Redirect::temporary(&url))
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
    let (posts, total) = JournalShareLink::find_public_posts_for_link(
        &link,
        &journal,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;

    Ok(Json(PublicSharedJournalBootstrapResponse {
        share_link: PublicSharedJournalShareLinkResponse {
            id: link.id,
            scoped_post_id: link.scoped_post_id,
            expires_at: link.expires_at,
        },
        journal: build_public_journal_response(journal, owner),
        posts: PaginatedResponse::new(
            posts
                .into_iter()
                .map(|post| {
                    PublicSharedJournalPostResponse::from_post(post, link.id, &params.token, &pool)
                })
                .collect::<Result<Vec<_>, _>>()?,
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
    let (link, journal) = JournalShareLink::authorize(share_link_id, &params.token, &pool)?;
    let (posts, total) = JournalShareLink::find_public_posts_for_link(
        &link,
        &journal,
        pagination.offset,
        pagination.limit,
        &pool,
    )?;

    Ok(Json(PaginatedResponse::new(
        posts
            .into_iter()
            .map(|post| {
                PublicSharedJournalPostResponse::from_post(post, share_link_id, &params.token, &pool)
            })
            .collect::<Result<Vec<_>, _>>()?,
        pagination,
        total,
    )))
}

#[debug_handler]
pub async fn get_shared_journal_asset_route(
    Extension(pool): Extension<DbPool>,
    Path((share_link_id, asset_id)): Path<(Uuid, Uuid)>,
    Query(params): Query<PublicShareLinkAssetQuery>,
) -> Result<Redirect, PpdcError> {
    authorize_shared_journal_asset_redirect(share_link_id, asset_id, &params.token, &pool).await
}
