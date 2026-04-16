use axum::{
    extract::DefaultBodyLimit,
    http::{Method, StatusCode},
    middleware::from_fn,
    response::IntoResponse,
    routing::{delete, get, patch, post, put, Router},
};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
};

use crate::entities_v2::{
    analysis_summary, asset, element,
    error::{ErrorType, PpdcError},
    journal, journal_grant, journal_share_link, landmark, landscape_analysis, lens, llm_call,
    mailer, message, post, post_grant, reference, relationship, trace, trace_mirror,
    transcription, usage_event, user, user_post_state, user_secure_action,
};
use crate::sessions_service;

pub fn create_router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(vec![
            Method::OPTIONS,
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers(Any);

    let users_router = Router::new()
        .route("/", get(user::get_users))
        .route("/search", get(user::get_user_search_route))
        .route("/:id", get(user::get_user_route).put(user::put_user_route))
        .route("/:id/posts", get(post::get_user_posts_route))
        .route(
            "/:id/analysis",
            post(landscape_analysis::post_analysis_route)
                .get(landscape_analysis::get_last_analysis_route),
        )
        .route("/:id/lens", get(lens::get_user_lenses_route))
        .route("/:id/traces", get(trace::get_all_traces_for_user_route))
        .route("/:id/journals", get(journal::get_user_journals_route))
        .route("/:id/heatmaps", get(trace::get_user_heatmap_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let mentors_router = Router::new()
        .route("/", get(user::get_mentors_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let admin_router = Router::new()
        .route(
            "/recent_activity",
            get(user::get_admin_recent_user_activity_route),
        )
        .route(
            "/service_users",
            get(user::get_admin_service_users_route).post(user::post_admin_service_user_route),
        )
        .route(
            "/service_users/:id",
            get(user::get_admin_service_user_route).put(user::put_admin_service_user_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let traces_router = Router::new()
        .route("/drafts", get(trace::get_trace_drafts_route))
        .route(
            "/:id",
            get(trace::get_trace_route)
                .put(trace::put_trace_route)
                .patch(trace::patch_trace_route),
        )
        .route(
            "/:id/posts",
            get(post::get_trace_posts_route).post(post::post_trace_post_route),
        )
        .route("/:id/extend_timeout", post(trace::post_trace_extend_timeout_route))
        .route(
            "/:id/assets",
            post(trace::post_trace_asset_route)
                .layer(DefaultBodyLimit::max(30 * 1024 * 1024)),
        )
        .route(
            "/:id/attachments",
            get(trace::get_trace_attachments_route)
                .post(trace::post_trace_attachment_route)
                .layer(DefaultBodyLimit::max(30 * 1024 * 1024)),
        )
        .route(
            "/:trace_id/attachments/:attachment_id",
            delete(trace::delete_trace_attachment_route),
        )
        .route("/:id/analysis", get(trace::get_trace_analysis_route))
        .route(
            "/:id/messages",
            get(trace::get_trace_messages_route).post(trace::post_trace_message_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let posts_router = Router::new()
        .route("/drafts", get(post::get_post_drafts_route))
        .route("/", get(post::get_posts_route).post(post::post_post_route))
        .route("/:id", get(post::get_post_route).put(post::put_post_route))
        .route("/:id/attachments", get(post::get_post_attachments_route))
        .route("/:id/seen_by", get(post::get_post_seen_by_route))
        .route(
            "/:id/messages",
            get(message::get_post_messages_route).post(message::post_post_message_route),
        )
        .route(
            "/:id/grants",
            get(post_grant::get_post_grants_route).post(post_grant::post_post_grant_route),
        )
        .route(
            "/:post_id/grants/:grant_id",
            delete(post_grant::delete_post_grant_route),
        )
        .route("/users/:id", get(post::get_user_posts_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let assets_router = Router::new()
        .route(
            "/",
            post(asset::post_asset_route).layer(DefaultBodyLimit::max(30 * 1024 * 1024)),
        )
        .route("/:id/url", get(asset::get_asset_signed_url_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let journals_router = Router::new()
        .route("/", post(journal::post_journal_route))
        .route("/shared/recent", get(journal::get_recent_shared_journals_route))
        .route("/shared", get(journal::get_shared_journals_route))
        .route(
            "/:id/draft",
            get(trace::get_journal_draft_route).post(trace::post_journal_draft_route),
        )
        .route(
            "/:id",
            get(journal::get_journal_route).put(journal::put_journal_route),
        )
        .route("/:id/share_links", post(journal_share_link::post_journal_share_link_route))
        .route(
            "/:journal_id/share_links/:share_link_id",
            delete(journal_share_link::delete_journal_share_link_route),
        )
        .route("/:id/exports", post(journal::post_journal_export_route))
        .route(
            "/:id/grants",
            get(journal_grant::get_journal_grants_route)
                .post(journal_grant::post_journal_grant_route),
        )
        .route("/:id/posts", get(post::get_journal_posts_route))
        .route(
            "/:journal_id/grants/:grant_id",
            delete(journal_grant::delete_journal_grant_route),
        )
        .route("/:id/traces", get(trace::get_traces_for_journal_route))
        .route("/:id/imports", post(journal::post_journal_import_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let relationships_router = Router::new()
        .route("/followers", get(relationship::get_followers_route))
        .route("/following", get(relationship::get_following_route))
        .route(
            "/requests/incoming",
            get(relationship::get_incoming_relationship_requests_route),
        )
        .route(
            "/requests/outgoing",
            get(relationship::get_outgoing_relationship_requests_route),
        )
        .route(
            "/",
            get(relationship::get_relationships_route).post(relationship::post_relationship_route),
        )
        .route("/:id", put(relationship::put_relationship_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let transcriptions_router = Router::new()
        .route("/", post(transcription::post_transcription_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let analysis_router = Router::new()
        .route(
            "/",
            get(landscape_analysis::get_current_lens_analysis_route)
                .post(landscape_analysis::post_analysis_route),
        )
        .route(
            "/:id",
            delete(landscape_analysis::delete_analysis_route)
                .get(landscape_analysis::get_analysis_route),
        )
        .route(
            "/:id/summaries",
            get(analysis_summary::get_analysis_summaries_route)
                .post(analysis_summary::post_analysis_summary_route),
        )
        .route(
            "/:id/landmarks",
            get(landscape_analysis::get_landmarks_route),
        )
        .route("/:id/elements", get(landscape_analysis::get_elements_route))
        .route(
            "/:id/traces",
            get(landscape_analysis::get_analysis_traces_route),
        )
        .route(
            "/:id/trace_mirrors",
            get(landscape_analysis::get_analysis_trace_mirrors_route),
        )
        .route(
            "/:id/parents",
            get(landscape_analysis::get_analysis_parents_route),
        )
        .route("/:id/messages", get(message::get_analysis_messages_route))
        .route("/:id/feedback", get(message::get_analysis_feedback_route))
        .route(
            "/:id/llm_calls",
            get(llm_call::get_llm_calls_by_analysis_id_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let internal_router = Router::new().route(
        "/run_pending_analyses",
        post(landscape_analysis::post_run_pending_analyses_route),
    )
    .route(
        "/replan_autoplay_lenses",
        post(landscape_analysis::post_replan_autoplay_lenses_route),
    )
    .route(
        "/process_pending_emails",
        post(mailer::post_process_pending_emails_route),
    );
    let analysis_summaries_router = Router::new()
        .route(
            "/:id",
            get(analysis_summary::get_analysis_summary_route)
                .put(analysis_summary::put_analysis_summary_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let landmarks_router = Router::new()
        .route("/:id", get(landmark::get_landmark_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let elements_router = Router::new()
        .route("/", get(element::get_elements_route))
        .route("/:id/landmarks", get(element::get_element_landmarks_route))
        .route("/:id/relations", get(element::get_element_relations_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let lens_router = Router::new()
        .route("/", post(lens::post_lens_route))
        .route("/:id/analysis", get(lens::get_lens_analysis_route))
        .route(
            "/:id/aggregates/week_events",
            get(lens::get_lens_week_events_route),
        )
        .route("/:id/retry", post(lens::post_lens_retry_route))
        .route(
            "/:id",
            delete(lens::delete_lens_route).put(lens::put_lens_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let llm_calls_router = Router::new()
        .route("/", get(llm_call::get_llm_calls_route))
        .route("/:id", get(llm_call::get_llm_call_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let messages_router = Router::new()
        .route(
            "/",
            get(message::get_messages_route).post(message::post_message_route),
        )
        .route(
            "/:id",
            get(message::get_message_route).put(message::put_message_route),
        )
        .route("/:id/seen", patch(message::patch_message_seen_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let trace_mirrors_router = Router::new()
        .route("/", get(trace_mirror::get_user_trace_mirrors_route))
        .route("/:id", get(trace_mirror::get_trace_mirror_route))
        .route(
            "/:id/references",
            get(reference::get_trace_mirror_references_route),
        )
        .route(
            "/landscape/:landscape_id",
            get(trace_mirror::get_trace_mirrors_by_landscape_route),
        )
        .route(
            "/trace/:trace_id",
            get(trace_mirror::get_trace_mirrors_by_trace_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let sessions_router = Router::new().route(
        "/",
        get(sessions_service::get_session_route)
            .post(sessions_service::post_session_route)
            .delete(sessions_service::delete_session_route),
    );
    let user_secure_actions_router = Router::new()
        .route("/", post(user_secure_action::post_user_secure_action_route))
        .route(
            "/consume",
            post(user_secure_action::post_user_secure_action_consume_route),
        );
    let usage_events_router = Router::new()
        .route("/", post(usage_event::post_usage_event_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let user_post_states_router = Router::new()
        .route("/", post(user_post_state::post_user_post_state_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let shared_router = Router::new()
        .route("/journals/:id", get(journal_share_link::get_shared_journal_route))
        .route(
            "/journals/:id/posts",
            get(journal_share_link::get_shared_journal_posts_route),
        );

    Router::new()
        .route("/users", post(user::post_user))
        .nest("/users", users_router)
        .nest("/mentors", mentors_router)
        .nest("/admin", admin_router)
        .nest("/traces", traces_router)
        .nest("/posts", posts_router)
        .nest("/assets", assets_router)
        .nest("/journals", journals_router)
        .nest("/transcriptions", transcriptions_router)
        .nest("/sessions", sessions_router)
        .nest("/user_secure_actions", user_secure_actions_router)
        .nest("/user_post_states", user_post_states_router)
        .nest("/usage_events", usage_events_router)
        .nest("/analysis", analysis_router)
        .nest("/internal", internal_router)
        .nest("/analysis_summaries", analysis_summaries_router)
        .nest("/lens", lens_router)
        .nest("/llm_calls", llm_calls_router)
        .nest("/messages", messages_router)
        .nest("/relationships", relationships_router)
        .nest("/landmarks", landmarks_router)
        .nest("/elements", elements_router)
        .nest("/trace_mirrors", trace_mirrors_router)
        .nest("/shared", shared_router)
        .fallback(fallback_handler)
        .route("/", get(root_handler))
        .layer(from_fn(sessions_service::add_session_to_request))
        .nest_service("/public", ServeDir::new("public"))
        .layer(cors)
}

async fn fallback_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        PpdcError::new(404, ErrorType::ApiError, "404 Not Found".to_string()),
    )
}
async fn root_handler() -> impl IntoResponse {
    (StatusCode::OK, "Ok")
}
