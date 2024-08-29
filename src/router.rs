use axum::{
    routing::{post, get, put, Router},
    middleware::from_fn,
    http::{StatusCode, Method},
    response::{IntoResponse},
};
use tower_http::{services::ServeDir, cors::{CorsLayer, Any}};

use crate::http::{HttpRequest, HttpResponse};
use crate::entities::{user::self,
    interaction::{interaction_routes as interaction},
    error::PpdcError,
    comment,
    resource
};
use crate::sessions_service;
use crate::file_converter;
use crate::entities::resource_relation;
use crate::link_preview;


pub fn create_router() -> Router {

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(vec![Method::GET, Method::POST, Method::PUT])
        .allow_headers(Any);

    let users_router = Router::new()
        .route("/", get(user::get_users).post(user::post_user))
        .route("/:id", get(user::get_user_route).put(user::put_user_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let resources_router = Router::new()
        .route("/", get(resource::get_resources_route).post(resource::post_resource_route))
        .route("/:id", get(resource::get_resource_route).put(resource::put_resource_route))
        .route("/:id/author_interaction", get(resource::get_resource_author_interaction_route))
        .route("/:id/interactions", get(interaction::get_interactions_for_resource_route)
            .post(interaction::post_interaction_for_resource))
        .route("/:id/bibliography", get(resource_relation::get_resource_relations_for_resource_route))
        .route("/:id/usages", get(resource_relation::get_targets_for_resource_route))
        .route("/:id/comments", get(comment::get_comments_for_resource).post(comment::post_comment_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let interactions_router = Router::new()
        .route("/", get(interaction::get_interactions_route))
        .route("/:id", put(interaction::put_interaction_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let relations_router = Router::new()
        .route("/thought_input_usages", post(resource_relation::post_resource_relation_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let comments_router = Router::new()
        .route("/:id", put(comment::put_comment))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let sessions_router = Router::new()
        .route("/", get(sessions_service::get_session_route).post(sessions_service::post_session_route));

    Router::new()
        .nest("/users", users_router)
        .nest("/resources", resources_router)
        .nest("/interactions", interactions_router)
        .nest("/", relations_router)
        .nest("/comments", comments_router)
        .nest("/sessions", sessions_router)
        .route("/link_preview", post(link_preview::post_preview_route))
        .route("/file_conversion", post(file_converter::post_file_conversion_route))
        .fallback(fallback_handler)
        .route("/", get(root_handler))
        .layer(from_fn(sessions_service::add_session_to_request))
        .nest_service("/public", ServeDir::new("public"))
        .layer(cors)
}

async fn fallback_handler() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "404 Not Found")
}
async fn root_handler() -> impl IntoResponse {
    (StatusCode::OK, "Ok")
}
