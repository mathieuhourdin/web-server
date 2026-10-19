use axum::{
    extract::DefaultBodyLimit,
    http::{request::Parts as RequestParts, HeaderName, HeaderValue, Method, StatusCode},
    middleware::from_fn,
    response::IntoResponse,
    routing::Router,
};
use regex::Regex;
use tower_http::{
    cors::{AllowHeaders, AllowOrigin, CorsLayer},
    services::ServeDir,
};

use crate::api_schema::{self, delete, get, patch, post, put, DocumentedRoutes};
use crate::entities_v2::platform_infra::automation;
use crate::entities_v2::platform_infra::platform_settings;
use crate::entities_v2::{
    album, analysis_summary, asset, audience, content_report, device, document, element,
    error::{ErrorType, PpdcError},
    feed, journal, journal_share_link, landmark, landscape_analysis, lens, llm_call, mailer,
    message, post, post_grant, reference, relationship, trace, trace_mirror, trace_search,
    transcription, url_preview, usage_event, user, user_block, user_post_state, user_secure_action,
    wal, writing_thread,
};
use crate::{environment, logging, sessions_service};

pub fn create_router() -> Router {
    let allow_origin_regex =
        Regex::new(&environment::get_allow_origin()).expect("ALLOW_ORIGIN must be a valid regex");
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(
            move |origin: &HeaderValue, _request_parts: &RequestParts| {
                origin
                    .to_str()
                    .map(|origin| allow_origin_regex.is_match(origin))
                    .unwrap_or(false)
            },
        ))
        .allow_methods(vec![
            Method::OPTIONS,
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers(AllowHeaders::mirror_request())
        .expose_headers([HeaderName::from_static(logging::REQUEST_ID_HEADER)])
        .allow_credentials(true);

    let users_router = Router::new()
        .api_route("/users", "/", get(user::get_users))
        .api_route("/users", "/suggested", get(user::get_suggested_users_route))
        .api_route("/users", "/search", get(user::get_user_search_route))
        .api_route(
            "/users",
            "/:id",
            get(user::get_user_route)
                .put(user::put_user_route)
                .patch(user::patch_user_route)
                .delete(user::delete_user_account_route),
        )
        .api_route(
            "/users",
            "/:id/profile_picture_asset",
            post(user::post_user_profile_picture_asset_route)
                .layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .api_route("/users", "/:id/albums", get(album::get_user_albums_route))
        .api_route(
            "/users",
            "/:id/documents",
            get(document::get_user_documents_route),
        )
        .api_route(
            "/users",
            "/:id/content_counts",
            get(user::get_user_content_counts_route),
        )
        .api_route("/users", "/:id/posts", get(post::get_user_posts_route))
        .api_route(
            "/users",
            "/:id/analysis",
            post(landscape_analysis::post_analysis_route)
                .get(landscape_analysis::get_last_analysis_route),
        )
        .api_route("/users", "/:id/lens", get(lens::get_user_lenses_route))
        .api_route(
            "/users",
            "/:id/traces",
            get(trace::get_all_traces_for_user_route),
        )
        .api_route(
            "/users",
            "/:id/journals/shared",
            get(journal::get_user_recent_shared_journals_route),
        )
        .api_route(
            "/users",
            "/:id/journals",
            get(journal::get_user_journals_route),
        )
        .api_route(
            "/users",
            "/:id/heatmaps",
            get(trace::get_user_heatmap_route),
        )
        .api_route(
            "/users",
            "/:id/emoji_heatmaps",
            get(analysis_summary::get_user_emoji_heatmap_route),
        )
        .api_route(
            "/users",
            "/:id/closest_followers",
            get(user::get_closest_followers_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom))
        .layer(from_fn(sessions_service::auth_marker_cookie_middleware));

    let mentors_router = Router::new()
        .api_route("/mentors", "/", get(user::get_mentors_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let admin_analytics_router = Router::new()
        .api_route(
            "/admin/analytics",
            "/summary",
            get(user::get_admin_analytics_summary_route),
        )
        .api_route(
            "/admin/analytics",
            "/platform_series",
            get(user::get_admin_platform_series_route),
        )
        .api_route(
            "/admin/analytics",
            "/users",
            get(user::get_admin_analytics_users_route),
        )
        .api_route(
            "/admin/analytics",
            "/users/:id/series",
            get(user::get_admin_user_series_route),
        )
        .api_route(
            "/admin/analytics",
            "/platform_overview",
            get(user::get_admin_platform_overview_route),
        )
        .api_route(
            "/admin/analytics",
            "/recent_activity",
            get(user::get_admin_recent_user_activity_route),
        );

    let admin_router = Router::new()
        .api_route(
            "/admin",
            "/automation/clients",
            get(automation::list_clients).post(automation::create_client),
        )
        .api_route(
            "/admin",
            "/automation/clients/:id",
            delete(automation::revoke_client),
        )
        .api_route(
            "/admin",
            "/automation/clients/:id/token",
            post(automation::rotate_client),
        )
        .api_route(
            "/admin",
            "/mobile/update_policy",
            get(platform_settings::get_admin_mobile_update_policy_route)
                .put(platform_settings::put_admin_mobile_update_policy_route),
        )
        .nest("/analytics", admin_analytics_router)
        .api_route("/admin", "/users", get(user::get_admin_users_route))
        .api_route(
            "/admin",
            "/users/:id/ai_features",
            patch(user::patch_admin_user_ai_features_route),
        )
        .api_route(
            "/admin",
            "/users/:id/llm_costs",
            get(user::get_admin_user_llm_costs_route),
        )
        .api_route(
            "/admin",
            "/users/:id",
            delete(user::delete_admin_user_route),
        )
        .api_route(
            "/admin",
            "/content_reports",
            get(content_report::get_admin_content_reports_route),
        )
        .api_route(
            "/admin",
            "/content_reports/:id",
            get(content_report::get_admin_content_report_route),
        )
        .api_route(
            "/admin",
            "/service_users",
            get(user::get_admin_service_users_route).post(user::post_admin_service_user_route),
        )
        .api_route(
            "/admin",
            "/service_users/:id",
            get(user::get_admin_service_user_route).put(user::put_admin_service_user_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let traces_router = Router::new()
        .api_route("/traces", "/:id/plume", put(trace::plume::put).delete(trace::plume::delete))
        .api_route("/traces", "/:id/plumes", get(trace::plume::readers))
        .api_route(
            "/traces",
            "/:id/writing_threads",
            get(writing_thread::routes::for_trace),
        )
        .api_route("/traces", "/drafts", get(trace::get_trace_drafts_route))
        .api_route(
            "/traces",
            "/search",
            get(trace_search::get_trace_search_route),
        )
        .api_route(
            "/traces",
            "/:id/edit_origin",
            get(trace::get_trace_edit_origin_route),
        )
        .api_route("/traces", "/:id/albums", get(album::get_trace_albums_route))
        .api_route(
            "/traces",
            "/:id/complements",
            get(trace::get_trace_complements_route).post(trace::post_trace_complement_route),
        )
        .api_route(
            "/traces",
            "/:id",
            get(trace::get_trace_route)
                .put(trace::put_trace_route)
                .patch(trace::patch_trace_route),
        )
        .api_route(
            "/traces",
            "/:id/post",
            get(post::get_trace_post_route)
                .put(post::put_trace_post_route)
                .delete(post::delete_trace_post_route),
        )
        .api_route(
            "/traces",
            "/:id/extend_timeout",
            post(trace::post_trace_extend_timeout_route),
        )
        .api_route("/traces", "/:id/seen", put(trace::put_trace_seen_route))
        .api_route(
            "/traces",
            "/:id/seen_by",
            get(trace::get_trace_seen_by_route),
        )
        .api_route(
            "/traces",
            "/:id/conversations",
            get(trace::get_trace_conversations_route),
        )
        .api_route(
            "/traces",
            "/:id/assets",
            post(trace::post_trace_asset_route).layer(DefaultBodyLimit::max(30 * 1024 * 1024)),
        )
        .api_route(
            "/traces",
            "/:id/source_assets",
            get(trace::get_trace_source_assets_route)
                .post(trace::post_trace_source_asset_route)
                .put(trace::put_trace_source_assets_order_route)
                .layer(DefaultBodyLimit::max(30 * 1024 * 1024)),
        )
        .api_route(
            "/traces",
            "/:trace_id/transcription_jobs",
            get(trace::get_transcription_jobs_route).post(trace::post_transcription_job_route),
        )
        .api_route(
            "/traces",
            "/:trace_id/transcription_jobs/:job_id",
            get(trace::get_transcription_job_route),
        )
        .api_route(
            "/traces",
            "/:trace_id/transcription_jobs/:job_id/confirm",
            post(trace::confirm_transcription_job_route),
        )
        .api_route(
            "/traces",
            "/:trace_id/source_assets/:source_asset_id",
            delete(trace::delete_trace_source_asset_route),
        )
        .api_route(
            "/traces",
            "/:id/attachments",
            get(trace::get_trace_attachments_route)
                .post(trace::post_trace_attachment_route)
                .layer(DefaultBodyLimit::max(30 * 1024 * 1024)),
        )
        .api_route(
            "/traces",
            "/:id/attachments/documents",
            post(trace::post_trace_document_attachment_route),
        )
        .api_route(
            "/traces",
            "/:trace_id/attachments/:attachment_id",
            delete(trace::delete_trace_attachment_route),
        )
        .api_route(
            "/traces",
            "/:id/analysis",
            get(trace::get_trace_analysis_route),
        )
        .api_route(
            "/traces",
            "/:id/messages",
            get(trace::get_trace_messages_route).post(trace::post_trace_message_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let posts_router = Router::new()
        .api_route(
            "/posts",
            "/:id/audiences",
            get(audience::routes::get_post_audiences),
        )
        .api_route(
            "/posts",
            "/:post_id/audiences/:audience_id",
            put(audience::routes::put_post_audience).delete(audience::routes::delete_post_audience),
        )
        .api_route("/posts", "/drafts", get(post::get_post_drafts_route))
        .api_route("/posts", "/feed", get(feed::get_feed_route))
        .api_route(
            "/posts",
            "/",
            get(post::get_posts_route).post(post::post_post_route),
        )
        .api_route(
            "/posts",
            "/:id",
            get(post::get_post_route).put(post::put_post_route),
        )
        .api_route(
            "/posts",
            "/:id/notification_settings",
            get(post::notifications::get_notification_settings),
        )
        .api_route(
            "/posts",
            "/:id/attachments",
            get(post::get_post_attachments_route),
        )
        .api_route("/posts", "/:id/seen_by", get(post::get_post_seen_by_route))
        .api_route(
            "/posts",
            "/:id/messages",
            get(message::get_post_messages_route).post(message::post_post_message_route),
        )
        .api_route(
            "/posts",
            "/:id/grants",
            get(post_grant::get_post_grants_route).post(post_grant::post_post_grant_route),
        )
        .api_route(
            "/posts",
            "/:post_id/grants/:grant_id",
            delete(post_grant::delete_post_grant_route),
        )
        .api_route("/posts", "/users/:id", get(post::get_user_posts_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let albums_router = Router::new()
        .api_route("/albums", "/", post(album::post_album_route))
        .api_route(
            "/albums",
            "/:id",
            get(album::get_album_route).put(album::put_album_route),
        )
        .api_route(
            "/albums",
            "/:id/post",
            get(post::get_album_post_route).put(post::put_album_post_route),
        )
        .api_route(
            "/albums",
            "/:id/assets",
            post(album::post_album_asset_route).layer(DefaultBodyLimit::max(30 * 1024 * 1024)),
        )
        .api_route(
            "/albums",
            "/:id/items",
            get(album::get_album_items_route).post(album::post_album_item_route),
        )
        .api_route(
            "/albums",
            "/:album_id/items/:item_id",
            delete(album::delete_album_item_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let assets_router = Router::new()
        .api_route(
            "/assets",
            "/",
            post(asset::post_asset_route).layer(DefaultBodyLimit::max(30 * 1024 * 1024)),
        )
        .api_route(
            "/assets",
            "/:id/url",
            get(asset::get_asset_signed_url_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let documents_router = Router::new()
        .api_route("/documents", "/", post(document::post_document_route))
        .api_route(
            "/documents",
            "/:id",
            get(document::get_document_route).put(document::put_document_route),
        )
        .api_route(
            "/documents",
            "/:id/cover_image_asset",
            post(document::post_document_cover_image_asset_route)
                .layer(DefaultBodyLimit::max(10 * 1024 * 1024)),
        )
        .api_route(
            "/documents",
            "/:id/post",
            get(post::get_document_post_route).put(post::put_document_post_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let journals_router = Router::new()
        .api_route(
            "/journals",
            "/:id/writing_threads",
            get(writing_thread::routes::for_journal),
        )
        .api_route(
            "/journals",
            "/:id/default_audiences",
            get(audience::routes::get_journal_defaults).put(audience::routes::put_journal_defaults),
        )
        .api_route("/journals", "/", post(journal::post_journal_route))
        .api_route(
            "/journals",
            "/shared/recent",
            get(journal::get_recent_shared_journals_route),
        )
        .api_route(
            "/journals",
            "/shared",
            get(journal::get_shared_journals_route),
        )
        .api_route(
            "/journals",
            "/:id/draft",
            get(trace::get_journal_draft_route)
                .post(trace::post_journal_draft_route)
                .patch(trace::patch_journal_draft_route),
        )
        .api_route(
            "/journals",
            "/:id",
            get(journal::get_journal_route).put(journal::put_journal_route),
        )
        .api_route(
            "/journals",
            "/:id/share_links",
            post(journal_share_link::post_journal_share_link_route),
        )
        .api_route(
            "/journals",
            "/:journal_id/share_links/:share_link_id",
            delete(journal_share_link::delete_journal_share_link_route),
        )
        .api_route(
            "/journals",
            "/:id/exports",
            post(journal::post_journal_export_route),
        )
        .api_route(
            "/journals",
            "/:id/audience",
            get(journal::get_journal_audience_route),
        )
        .api_route(
            "/journals",
            "/:journal_id/audience/:user_id/traces",
            get(journal::get_journal_audience_user_traces_route),
        )
        .api_route(
            "/journals",
            "/:id/posts",
            get(post::get_journal_posts_route),
        )
        .api_route(
            "/journals",
            "/:id/messages",
            get(journal::get_journal_messages_route).post(journal::post_journal_message_route),
        )
        .api_route(
            "/journals",
            "/:id/traces/finalized",
            post(trace::post_finalized_journal_trace_route),
        )
        .api_route(
            "/journals",
            "/:id/traces",
            get(trace::get_traces_for_journal_route),
        )
        .api_route(
            "/journals",
            "/:id/imports",
            post(journal::post_journal_import_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let audiences_router = Router::new()
        .api_route(
            "/audiences",
            "/",
            get(audience::routes::get_audiences).post(audience::routes::post_audience),
        )
        .api_route(
            "/audiences",
            "/history_reviews",
            get(audience::routes::get_pending_reviews),
        )
        .api_route(
            "/audiences",
            "/:id",
            get(audience::routes::get_audience)
                .patch(audience::routes::patch_audience)
                .delete(audience::routes::delete_audience),
        )
        .api_route(
            "/audiences",
            "/:id/members",
            get(audience::routes::get_members),
        )
        .api_route(
            "/audiences",
            "/:id/members/:user_id",
            get(audience::routes::get_member)
                .put(audience::routes::put_member)
                .patch(audience::routes::patch_member)
                .delete(audience::routes::delete_member),
        )
        .api_route(
            "/audiences",
            "/:id/members/:user_id/history_summary",
            get(audience::routes::get_history_summary),
        )
        .api_route(
            "/audiences",
            "/:id/members/:user_id/history_posts",
            get(audience::routes::get_history_posts),
        )
        .api_route(
            "/audiences",
            "/:id/members/:user_id/history_decision",
            post(audience::routes::post_history_decision),
        )
        .api_route(
            "/audiences",
            "/:id/members/:user_id/history_review",
            post(audience::routes::post_history_review),
        )
        .api_route("/audiences", "/:id/posts", get(audience::routes::get_posts))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let relationships_router = Router::new()
        .api_route(
            "/relationships",
            "/followers",
            get(relationship::get_followers_route),
        )
        .api_route(
            "/relationships",
            "/following",
            get(relationship::get_following_route),
        )
        .api_route(
            "/relationships",
            "/requests/incoming",
            get(relationship::get_incoming_relationship_requests_route),
        )
        .api_route(
            "/relationships",
            "/requests/outgoing",
            get(relationship::get_outgoing_relationship_requests_route),
        )
        .api_route(
            "/relationships",
            "/",
            get(relationship::get_relationships_route).post(relationship::post_relationship_route),
        )
        .api_route(
            "/relationships",
            "/:id",
            put(relationship::put_relationship_route)
                .delete(relationship::delete_relationship_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let transcriptions_router = Router::new()
        .api_route(
            "/transcriptions",
            "/",
            post(transcription::post_transcription_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let analysis_router = Router::new()
        .api_route(
            "/analysis",
            "/",
            get(landscape_analysis::get_current_lens_analysis_route)
                .post(landscape_analysis::post_analysis_route),
        )
        .api_route(
            "/analysis",
            "/:id",
            delete(landscape_analysis::delete_analysis_route)
                .get(landscape_analysis::get_analysis_route),
        )
        .api_route(
            "/analysis",
            "/:id/summaries",
            get(analysis_summary::get_analysis_summaries_route)
                .post(analysis_summary::post_analysis_summary_route),
        )
        .api_route(
            "/analysis",
            "/:id/landmarks",
            get(landscape_analysis::get_landmarks_route),
        )
        .api_route(
            "/analysis",
            "/:id/elements",
            get(landscape_analysis::get_elements_route),
        )
        .api_route(
            "/analysis",
            "/:id/traces",
            get(landscape_analysis::get_analysis_traces_route),
        )
        .api_route(
            "/analysis",
            "/:id/trace_mirrors",
            get(landscape_analysis::get_analysis_trace_mirrors_route),
        )
        .api_route(
            "/analysis",
            "/:id/parents",
            get(landscape_analysis::get_analysis_parents_route),
        )
        .api_route(
            "/analysis",
            "/:id/messages",
            get(message::get_analysis_messages_route),
        )
        .api_route(
            "/analysis",
            "/:id/feedback",
            get(message::get_analysis_feedback_route),
        )
        .api_route(
            "/analysis",
            "/:id/llm_calls",
            get(llm_call::get_llm_calls_by_analysis_id_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let internal_router = Router::new()
        .api_route(
            "/internal",
            "/run_pending_analyses",
            post(landscape_analysis::post_run_pending_analyses_route),
        )
        .api_route(
            "/internal",
            "/replan_autoplay_lenses",
            post(landscape_analysis::post_replan_autoplay_lenses_route),
        )
        .api_route(
            "/internal",
            "/process_pending_emails",
            post(mailer::post_process_pending_emails_route),
        );
    let internal_router = internal_router.api_route(
        "/internal",
        "/generate_shared_journal_daily_digests",
        post(mailer::post_generate_shared_journal_daily_digests_route),
    );
    let analysis_summaries_router = Router::new()
        .api_route(
            "/analysis_summaries",
            "/:id",
            get(analysis_summary::get_analysis_summary_route)
                .put(analysis_summary::put_analysis_summary_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let landmarks_router = Router::new()
        .api_route("/landmarks", "/:id", get(landmark::get_landmark_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let elements_router = Router::new()
        .api_route("/elements", "/", get(element::get_elements_route))
        .api_route(
            "/elements",
            "/:id/landmarks",
            get(element::get_element_landmarks_route),
        )
        .api_route(
            "/elements",
            "/:id/relations",
            get(element::get_element_relations_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let lens_router = Router::new()
        .api_route("/lens", "/", post(lens::post_lens_route))
        .api_route("/lens", "/:id/analysis", get(lens::get_lens_analysis_route))
        .api_route(
            "/lens",
            "/:id/aggregates/week_events",
            get(lens::get_lens_week_events_route),
        )
        .api_route("/lens", "/:id/retry", post(lens::post_lens_retry_route))
        .api_route(
            "/lens",
            "/:id",
            delete(lens::delete_lens_route).put(lens::put_lens_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let llm_calls_router = Router::new()
        .api_route("/llm_calls", "/", get(llm_call::get_llm_calls_route))
        .api_route("/llm_calls", "/:id", get(llm_call::get_llm_call_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let messages_router = Router::new()
        .api_route(
            "/messages",
            "/",
            get(message::get_messages_route).post(message::post_message_route),
        )
        .api_route(
            "/messages",
            "/conversations",
            get(message::get_conversations_route),
        )
        .api_route(
            "/messages",
            "/conversations/:partner_id",
            get(message::get_conversation_thread_route),
        )
        .api_route(
            "/messages",
            "/:id",
            get(message::get_message_route).put(message::put_message_route),
        )
        .api_route(
            "/messages",
            "/:id/seen",
            put(message::put_message_seen_route).patch(message::patch_message_seen_route),
        )
        .api_route(
            "/messages",
            "/:id/reaction",
            put(message::put_message_reaction_route).delete(message::delete_message_reaction_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let trace_mirrors_router = Router::new()
        .api_route(
            "/trace_mirrors",
            "/",
            get(trace_mirror::get_user_trace_mirrors_route),
        )
        .api_route(
            "/trace_mirrors",
            "/:id",
            get(trace_mirror::get_trace_mirror_route),
        )
        .api_route(
            "/trace_mirrors",
            "/:id/references",
            get(reference::get_trace_mirror_references_route),
        )
        .api_route(
            "/trace_mirrors",
            "/landscape/:landscape_id",
            get(trace_mirror::get_trace_mirrors_by_landscape_route),
        )
        .api_route(
            "/trace_mirrors",
            "/trace/:trace_id",
            get(trace_mirror::get_trace_mirrors_by_trace_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let sessions_router = Router::new()
        .api_route(
            "/sessions",
            "/",
            get(sessions_service::get_session_route)
                .post(sessions_service::post_session_route)
                .delete(sessions_service::delete_session_route),
        )
        .api_route(
            "/sessions",
            "/google",
            post(sessions_service::post_google_session_route),
        )
        .api_route(
            "/sessions",
            "/apple",
            post(sessions_service::post_apple_session_route),
        );
    let devices_router = Router::new()
        .api_route("/devices", "/", get(device::get_devices_route))
        .api_route(
            "/devices",
            "/current",
            patch(device::patch_current_device_route),
        )
        .api_route("/devices", "/:id", delete(device::delete_device_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let user_secure_actions_router = Router::new()
        .api_route(
            "/user_secure_actions",
            "/",
            post(user_secure_action::post_user_secure_action_route),
        )
        .api_route(
            "/user_secure_actions",
            "/consume",
            post(user_secure_action::post_user_secure_action_consume_route),
        );
    let usage_events_router = Router::new()
        .api_route(
            "/usage_events",
            "/",
            post(usage_event::post_usage_event_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let url_previews_router = Router::new()
        .api_route(
            "/url_previews",
            "/",
            post(url_preview::post_url_preview_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let user_post_states_router = Router::new()
        .api_route(
            "/user_post_states",
            "/",
            post(user_post_state::post_user_post_state_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let content_reports_router = Router::new()
        .api_route(
            "/content_reports",
            "/",
            post(content_report::post_content_report_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let shared_router = Router::new()
        .api_route(
            "/shared",
            "/journals/:id",
            get(journal_share_link::get_shared_journal_route),
        )
        .api_route(
            "/shared",
            "/journals/:id/assets/:asset_id",
            get(journal_share_link::get_shared_journal_asset_route),
        )
        .api_route(
            "/shared",
            "/journals/:id/posts",
            get(journal_share_link::get_shared_journal_posts_route),
        )
        .api_route(
            "/shared",
            "/journals/:id/traces",
            get(journal_share_link::get_shared_journal_traces_route),
        );
    let feed_router = Router::new()
        .api_route("/feed", "/", get(feed::get_feed_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));
    let me_router = Router::new()
        .api_route("/me", "/audience/:user_id/traces", get(trace::access::for_reader))
        .api_route("/me", "/account", delete(user::delete_my_account_route))
        .api_route(
            "/me",
            "/wal",
            get(wal::get_wal_route).post(wal::post_wal_route),
        )
        .api_route("/me", "/wals", get(wal::get_wals_route))
        .api_route("/me", "/wals/:wal_day_id", get(wal::get_wal_day_route))
        .api_route(
            "/me",
            "/wal/compilation",
            get(wal::get_wal_compilation_route).post(wal::post_wal_compilation_route),
        )
        .api_route("/me", "/wal/carryover", get(wal::get_wal_carryover_route))
        .api_route(
            "/me",
            "/wal/carryover/:projection_id/apply",
            post(wal::post_apply_wal_carryover_route),
        )
        .api_route(
            "/me",
            "/wal/carryover/:projection_id/resolution",
            post(wal::post_resolve_wal_carryover_route),
        )
        .api_route(
            "/me",
            "/traces/export",
            get(journal::get_all_my_traces_export_route),
        )
        .api_route("/me", "/blocks", get(user_block::get_my_blocks_route))
        .api_route(
            "/me",
            "/blocks/:user_id",
            put(user_block::put_my_block_route).delete(user_block::delete_my_block_route),
        )
        .api_route(
            "/me",
            "/unread_counts",
            get(user::get_me_unread_counts_route),
        )
        .api_route("/me", "/traces", get(trace::get_me_traces_route))
        .api_route("/me", "/feed", get(feed::get_my_feed_route))
        .api_route("/me", "/landmarks", get(landmark::get_me_landmarks_route))
        .api_route(
            "/me",
            "/high_level_projects",
            get(landmark::get_me_high_level_projects_route),
        )
        .api_route(
            "/me",
            "/auth_providers/google",
            post(sessions_service::post_google_provider_link_route)
                .delete(sessions_service::delete_google_provider_link_route),
        )
        .api_route(
            "/me",
            "/auth_providers/apple",
            post(sessions_service::post_apple_provider_link_route)
                .delete(sessions_service::delete_apple_provider_link_route),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let writing_threads_router = Router::new()
        .api_route("/writing_threads", "/", get(writing_thread::routes::list))
        .api_route("/writing_threads", "/:id", get(writing_thread::routes::get))
        .api_route(
            "/writing_threads",
            "/:id/traces",
            get(writing_thread::routes::traces),
        )
        .api_route(
            "/writing_threads",
            "/:id/generation",
            post(writing_thread::routes::generate),
        )
        .layer(from_fn(sessions_service::auth_middleware_custom));
    Router::new()
        .api_route("", "/api/schema", get(api_schema::schema))
        .api_route("", "/api/docs", get(api_schema::docs))
        .nest("/writing_threads", writing_threads_router)
        .api_route("", "/me/capabilities", get(automation::capabilities))
        .api_route("", "/automation/contract", get(automation::contract))
        .api_route(
            "",
            "/automation/demo_users",
            get(automation::list_demos).post(automation::create_demos),
        )
        .api_route(
            "",
            "/automation/demo_users/:id",
            get(automation::get_demo)
                .put(automation::update_demo)
                .delete(automation::delete_demo),
        )
        .api_route(
            "",
            "/automation/demo_users/:id/delegations",
            post(automation::delegate),
        )
        .api_route(
            "",
            "/mobile/version_policy",
            get(platform_settings::get_mobile_version_policy_route),
        )
        .api_route("", "/users", post(user::post_user))
        .api_route(
            "",
            "/users/google",
            post(sessions_service::post_google_user_route),
        )
        .api_route(
            "",
            "/users/apple",
            post(sessions_service::post_apple_user_route),
        )
        .nest("/users", users_router)
        .nest("/mentors", mentors_router)
        .nest("/admin", admin_router)
        .nest("/traces", traces_router)
        .nest("/posts", posts_router)
        .nest("/albums", albums_router)
        .nest("/assets", assets_router)
        .nest("/documents", documents_router)
        .nest("/journals", journals_router)
        .nest("/transcriptions", transcriptions_router)
        .nest("/sessions", sessions_router)
        .nest("/devices", devices_router)
        .nest("/user_secure_actions", user_secure_actions_router)
        .nest("/user_post_states", user_post_states_router)
        .nest("/content_reports", content_reports_router)
        .nest("/url_previews", url_previews_router)
        .nest("/usage_events", usage_events_router)
        .nest("/analysis", analysis_router)
        .nest("/internal", internal_router)
        .nest("/analysis_summaries", analysis_summaries_router)
        .nest("/lens", lens_router)
        .nest("/llm_calls", llm_calls_router)
        .nest("/messages", messages_router)
        .nest("/relationships", relationships_router)
        .nest("/audiences", audiences_router)
        .nest("/landmarks", landmarks_router)
        .nest("/elements", elements_router)
        .nest("/trace_mirrors", trace_mirrors_router)
        .nest("/feed", feed_router)
        .nest("/me", me_router)
        .nest("/shared", shared_router)
        .fallback(fallback_handler)
        .api_route("", "/", get(root_handler))
        .layer(from_fn(logging::request_context_middleware))
        .layer(from_fn(automation::gate))
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
