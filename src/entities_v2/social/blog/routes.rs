use super::{
    model::{
        self, Blog, BlogInput, PatchPublicationInput, PublicationResponse, UserPublicationResponse,
    },
    render, urls,
};
use crate::{
    api_schema::{get, put, DocumentedRoutes},
    db::DbPool,
    entities_v2::{
        asset::{Asset, AssetStatus},
        session::Session,
    },
    pagination::{PaginatedResponse, PaginationParams},
};
use axum::{
    extract::{Extension, Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Json, Router,
};
use uuid::Uuid;

pub fn management_router() -> Router {
    Router::new()
        .api_route("/blogs", "/", get(list).post(create))
        .api_route("/blogs", "/:id", get(read).put(update).delete(delete))
        .api_route("/blogs", "/:id/publications", get(list_publications))
        .api_route("/blogs", "/:id/traces/:source_id", put(publish_trace))
        .api_route("/blogs", "/:id/albums/:source_id", put(publish_album))
        .api_route(
            "/blogs",
            "/:id/publications/:publication_id",
            get(read_publication)
                .patch(patch_publication)
                .delete(unpublish),
        )
}
pub fn public_router() -> Router {
    Router::new()
        .api_route("/blog", "/_assets/hupo.svg", get(logo))
        .api_route("/blog", "/_assets/hupo.png", get(preview_logo))
        .api_route("/blog", "/_assets/share.js", get(share_script))
        .api_route("/blog", "/p/:short_code", get(short_link))
        .api_route("/blog", "/:slug", get(home))
        .api_route("/blog", "/:slug/posts/:post_id", get(article))
        .api_route("/blog", "/:slug/assets/:asset_id", get(blog_image))
        .api_route(
            "/blog",
            "/:slug/posts/:post_id/assets/:asset_id",
            get(post_image),
        )
}
async fn preview_logo() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        include_bytes!("../../../../public/assets/email-icon.png").as_slice(),
    )
}
async fn share_script() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/javascript; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        include_str!("../../../../public/assets/blog-share.js"),
    )
}
async fn logo() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; sandbox",
            ),
        ],
        include_str!("../../../../public/assets/icon.svg"),
    )
}
fn user(session: &Session) -> Result<Uuid, crate::entities_v2::error::PpdcError> {
    session
        .user_id
        .ok_or_else(crate::entities_v2::error::PpdcError::unauthorized)
}
type Error = crate::entities_v2::error::PpdcError;

async fn list(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Query(page): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<Blog>>, Error> {
    Ok(Json(model::list(user(&session)?, page.validate()?, &pool)?))
}
async fn create(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Json(input): Json<BlogInput>,
) -> Result<(StatusCode, Json<Blog>), Error> {
    Ok((
        StatusCode::CREATED,
        Json(model::save(None, user(&session)?, input, &pool)?),
    ))
}
async fn read(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<Json<Blog>, Error> {
    Ok(Json(model::owned(id, user(&session)?, &pool)?))
}
async fn delete(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Error> {
    let blog = model::owned(id, user(&session)?, &pool)?;
    model::delete(&blog, &pool)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn update(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Json(input): Json<BlogInput>,
) -> Result<Json<Blog>, Error> {
    Ok(Json(model::save(Some(id), user(&session)?, input, &pool)?))
}
async fn list_publications(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(id): Path<Uuid>,
    Query(page): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<PublicationResponse>>, Error> {
    let blog = model::owned(id, user(&session)?, &pool)?;
    let pagination = page.validate()?;
    let posts = model::posts(&blog, false, pagination, &pool)?;
    Ok(Json(PaginatedResponse::new(
        posts
            .items
            .into_iter()
            .map(|post| post.with_urls(&blog.slug))
            .collect(),
        pagination,
        posts.total,
    )))
}

pub async fn get_user_publications_route(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path(user_id): Path<Uuid>,
    Query(page): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<UserPublicationResponse>>, Error> {
    // Public blogs can use unrelated pseudonyms. Do not reveal their account
    // association through a user-level listing without an explicit product choice.
    let owner = user(&session)?;
    if owner != user_id {
        return Err(Error::unauthorized());
    }
    let pagination = page.validate()?;
    let posts = model::owner_publications(owner, pagination, &pool)?;
    Ok(Json(PaginatedResponse::new(
        posts
            .items
            .into_iter()
            .map(|post| post.with_urls())
            .collect(),
        pagination,
        posts.total,
    )))
}

async fn read_publication(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((id, post)): Path<(Uuid, Uuid)>,
) -> Result<Json<PublicationResponse>, Error> {
    let blog = model::owned(id, user(&session)?, &pool)?;
    Ok(Json(
        model::post(&blog, post, false, &pool)?.with_urls(&blog.slug),
    ))
}
pub(super) async fn patch_publication(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((id, publication)): Path<(Uuid, Uuid)>,
    Json(input): Json<PatchPublicationInput>,
) -> Result<Json<PublicationResponse>, Error> {
    tokio::task::spawn_blocking(move || {
        let blog = model::owned(id, user(&session)?, &pool)?;
        Ok(Json(
            model::patch_publication(&blog, publication, input, &pool)?.with_urls(&blog.slug),
        ))
    })
    .await
    .map_err(|_| model::error(500, "Unable to update publication ordering"))?
}
async fn publish_trace(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((id, source)): Path<(Uuid, Uuid)>,
) -> Result<Json<PublicationResponse>, Error> {
    let blog = model::owned(id, user(&session)?, &pool)?;
    Ok(Json(
        model::publish(&blog, source, false, &pool)?.with_urls(&blog.slug),
    ))
}
async fn publish_album(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((id, source)): Path<(Uuid, Uuid)>,
) -> Result<Json<PublicationResponse>, Error> {
    let blog = model::owned(id, user(&session)?, &pool)?;
    Ok(Json(
        model::publish(&blog, source, true, &pool)?.with_urls(&blog.slug),
    ))
}
async fn unpublish(
    Extension(pool): Extension<DbPool>,
    Extension(session): Extension<Session>,
    Path((id, post)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, Error> {
    let blog = model::owned(id, user(&session)?, &pool)?;
    model::unpublish(&blog, post, &pool)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn home(
    Extension(pool): Extension<DbPool>,
    Path(slug): Path<String>,
    Query(page): Query<PaginationParams>,
) -> Result<Response, Error> {
    let blog = model::public(&slug, &pool)?;
    let posts = model::posts(&blog, true, page.validate()?, &pool)?;
    Ok(render::home(&blog, &posts))
}
pub(super) async fn short_link(
    Extension(pool): Extension<DbPool>,
    Path(code): Path<String>,
) -> Response {
    let mut response =
        match tokio::task::spawn_blocking(move || model::short_target(&code, &pool)).await {
            Ok(Ok(target)) => {
                Redirect::temporary(&urls::publication(&target.slug, target.id)).into_response()
            }
            Ok(Err(error)) => error.into_response(),
            Err(_) => Error::new(
                500,
                crate::entities_v2::error::ErrorType::InternalError,
                "Unable to resolve publication link".into(),
            )
            .into_response(),
        };
    // Never cache either a redirect or a negative lookup: current publication
    // and source availability must be checked again on every new visit.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    response
        .headers_mut()
        .insert("x-robots-tag", "noindex".parse().unwrap());
    response
}
async fn article(
    Extension(pool): Extension<DbPool>,
    Path((slug, id)): Path<(String, Uuid)>,
    Query(page): Query<PaginationParams>,
) -> Result<Response, Error> {
    let blog = model::public(&slug, &pool)?;
    let post = model::post(&blog, id, true, &pool)?;
    let items = if post.source_album_id.is_some() {
        Some(model::subject_items(&blog, id, page.validate()?, &pool)?)
    } else {
        None
    };
    Ok(render::article(&blog, &post, items.as_ref()))
}
async fn image(asset: Uuid, owner: Uuid, pool: &DbPool) -> Result<Response, Error> {
    let asset = Asset::find(asset, pool)?;
    if asset.owner_user_id != owner
        || asset.status != AssetStatus::Ready
        || !model::safe_image(&asset.mime_type)
    {
        return Err(model::error(404, "Image not found"));
    }
    // Stable public URL, private storage. A previously issued URL can remain
    // valid for at most 60 seconds after unpublication; never persist it in HTML.
    let (url, _) = asset.signed_read_url(60).await?;
    let mut response = Redirect::temporary(&url).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert(header::REFERRER_POLICY, "no-referrer".parse().unwrap());
    Ok(response)
}
async fn blog_image(
    Extension(pool): Extension<DbPool>,
    Path((slug, asset)): Path<(String, Uuid)>,
) -> Result<Response, Error> {
    let blog = model::public(&slug, &pool)?;
    if blog.image_asset_id != Some(asset) {
        return Err(model::error(404, "Image not found"));
    }
    image(asset, blog.user_id, &pool).await
}
async fn post_image(
    Extension(pool): Extension<DbPool>,
    Path((slug, id, asset)): Path<(String, Uuid, Uuid)>,
) -> Result<Response, Error> {
    let blog = model::public(&slug, &pool)?;
    let post = model::post(&blog, id, true, &pool)?;
    if post.image_asset_id != Some(asset) {
        return Err(model::error(404, "Image not found"));
    }
    image(asset, blog.user_id, &pool).await
}
