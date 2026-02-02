use axum::{
    http::{Method, StatusCode},
    middleware::from_fn,
    response::IntoResponse,
    routing::{delete, get, post, put, Router},
};
use http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

use crate::entities::resource_relation;
use crate::entities::transcription;
use crate::entities::{
    comment, interaction::interaction_routes as interaction, resource, user,
};
use crate::entities_v2::{
    trace,
    landscape_analysis,
    llm_call,
    landmark,
    lens,
};
use crate::link_preview;
use crate::sessions_service;

pub fn create_router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(vec![Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([CONTENT_TYPE, ACCEPT, AUTHORIZATION]);

    let users_router = Router::new()
        .route("/", get(user::get_users).post(user::post_user))
        .route("/:id", get(user::get_user_route).put(user::put_user_route))
        .route("/:id/analysis", post(landscape_analysis::post_analysis_route).get(landscape_analysis::get_last_analysis_route))
        .route("/:id/landmarks", get(landmark::get_landmarks_for_user_route))
        .route("/:id/lens", get(lens::get_user_lenses_route))
        .route("/:id/traces", get(trace::get_all_traces_for_user_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let resources_router = Router::new()
        .route(
            "/",
            get(resource::get_resources_route).post(resource::post_resource_route),
        )
        .route(
            "/:id",
            get(resource::get_resource_route).put(resource::put_resource_route),
        )
        .route(
            "/:id/author_interaction",
            get(resource::get_resource_author_interaction_route),
        )
        .route(
            "/:id/interactions",
            get(interaction::get_interactions_for_resource_route)
                .post(interaction::post_interaction_for_resource),
        )
        .route(
            "/:id/bibliography",
            get(resource_relation::get_resource_relations_for_resource_route),
        )
        .route(
            "/:id/usages",
            get(resource_relation::get_targets_for_resource_route),
        )
        .route(
            "/:id/comments",
            get(comment::get_comments_for_resource).post(comment::post_comment_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let traces_router = Router::new()
        .route("/", post(trace::post_trace_route))
        .route("/:id", get(trace::get_trace_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let interactions_router = Router::new()
        .route("/", get(interaction::get_interactions_route))
        .route("/:id", put(interaction::put_interaction_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let relations_router = Router::new()
        .route(
            "/thought_input_usages",
            post(resource_relation::post_resource_relation_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let comments_router = Router::new()
        .route("/:id", put(comment::put_comment))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let transcriptions_router = Router::new()
        .route("/", post(transcription::post_transcription_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let analysis_router = Router::new()
        .route("/", post(landscape_analysis::post_analysis_route))
        .route("/:id", delete(landscape_analysis::delete_analysis_route).get(landscape_analysis::get_analysis_route))
        .route("/:id/landmarks", get(landscape_analysis::get_landmarks_route))
        .route("/:id/parents", get(landscape_analysis::get_analysis_parents_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let landmarks_router = Router::new()
        .route("/:id", get(landmark::get_landmark_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let lens_router = Router::new()
        .route("/", post(lens::post_lens_route))
        .route("/:id", delete(lens::delete_lens_route).put(lens::put_lens_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let llm_calls_router = Router::new()
        .route("/", get(llm_call::get_llm_calls_route))
        .route("/:id", get(llm_call::get_llm_call_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let sessions_router = Router::new().route(
        "/",
        get(sessions_service::get_session_route).post(sessions_service::post_session_route),
    );

    Router::new()
        .nest("/users", users_router)
        .nest("/resources", resources_router)
        .nest("/traces", traces_router)
        .nest("/interactions", interactions_router)
        .nest("/transcriptions", transcriptions_router)
        .nest("/", relations_router)
        .nest("/comments", comments_router)
        .nest("/sessions", sessions_router)
        .nest("/analysis", analysis_router)
        .nest("/lens", lens_router)
        .nest("/llm_calls", llm_calls_router)
        .nest("/landmarks", landmarks_router)
        .route("/link_preview", post(link_preview::post_preview_route))
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
