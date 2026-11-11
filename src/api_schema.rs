//! Route-first OpenAPI discovery. Payload schemas will be added incrementally.
//! The wrappers retain Axum's handlers/middleware and record documentation only.
use axum::{
    extract::{DefaultBodyLimit, Query},
    handler::Handler,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{MethodRouter, Router},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::{
    collections::BTreeMap,
    sync::{Mutex, OnceLock},
};

#[derive(Clone)]
struct Operation {
    method: &'static str,
    handler: &'static str,
}

fn registry() -> &'static Mutex<BTreeMap<String, Vec<Operation>>> {
    static ROUTES: OnceLock<Mutex<BTreeMap<String, Vec<Operation>>>> = OnceLock::new();
    ROUTES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub(crate) struct DescribedMethods {
    inner: MethodRouter,
    operations: Vec<Operation>,
}

macro_rules! methods {
    ($($method:ident => $verb:literal),*) => {
        $(pub(crate) fn $method<H,T>(handler:H)->DescribedMethods
        where H:Handler<T,()>, T:'static {
            DescribedMethods {
                inner: axum::routing::$method(handler),
                operations:vec![Operation {method:$verb,handler:std::any::type_name::<H>()}],
            }
        })*
        impl DescribedMethods {
            $(pub(crate) fn $method<H,T>(mut self,handler:H)->Self
            where H:Handler<T,()>, T:'static {
                self.inner=self.inner.$method(handler);
                self.operations.push(Operation {method:$verb,handler:std::any::type_name::<H>()});
                self
            })*
        }
    }
}
methods!(get=>"GET",post=>"POST",put=>"PUT",patch=>"PATCH",delete=>"DELETE");

impl DescribedMethods {
    pub(crate) fn layer(mut self, limit: DefaultBodyLimit) -> Self {
        self.inner = self.inner.layer(limit);
        self
    }
}

pub(crate) trait DocumentedRoutes {
    fn api_route(self, prefix: &str, path: &str, methods: DescribedMethods) -> Self;
}
impl DocumentedRoutes for Router {
    fn api_route(self, prefix: &str, path: &str, methods: DescribedMethods) -> Self {
        // Same joining rule as Axum 0.7's nested path router ("/" becomes prefix).
        let full_path = if prefix.is_empty() {
            path.to_string()
        } else if path == "/" {
            prefix.to_string()
        } else {
            format!("{prefix}{path}")
        };
        {
            let mut routes = registry()
                .lock()
                .expect("API documentation registry poisoned");
            let operations = routes.entry(full_path).or_default();
            for operation in methods.operations {
                if let Some(existing) = operations
                    .iter()
                    .find(|item| item.method == operation.method)
                {
                    assert_eq!(
                        existing.handler, operation.handler,
                        "Conflicting API documentation registration"
                    );
                } else {
                    operations.push(operation);
                }
            }
        }
        self.route(path, methods.inner)
    }
}

fn openapi_path(path: &str) -> String {
    path.split('/')
        .map(|segment| {
            if let Some(name) = segment.strip_prefix(':') {
                format!("{{{name}}}")
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn access(method: &str, path: &str) -> (&'static str, Value) {
    if path.starts_with("/admin/") {
        return (
            "Administrator session; handler role checks still apply.",
            json!([{"sessionBearer":[]}]),
        );
    }
    if path.starts_with("/internal/") {
        return (
            "Internal scheduler token; not available to agents.",
            json!([{"internalCronToken":[]}]),
        );
    }
    if path.starts_with("/automation/") || path == "/me/capabilities" {
        return ("Automation credential and effective scopes. Management requires the agent key, not a delegated session.",json!([{"agentBearer":[]}]));
    }
    if path.starts_with("/blog/") {
        return ("Anonymous public blog HTML or image. Blog, publication and current source must be publicly available.",json!([]));
    }
    if path.starts_with("/shared/") {
        return (
            "Public journal-sharing token, validated by the handler.",
            json!([{"journalShareToken":[]}]),
        );
    }
    if matches!(
        path,
        "/" | "/healthz" | "/api/schema" | "/api/docs" | "/mobile/version_policy"
    ) || (method == "POST"
        && matches!(
            path,
            "/users"
                | "/users/google"
                | "/users/apple"
                | "/sessions"
                | "/sessions/google"
                | "/sessions/apple"
        ))
        || (method == "GET" && path == "/sessions")
    {
        return ("No authenticated user session required. Invalid supplied credentials may still be rejected.",json!([]));
    }
    if method == "POST"
        && matches!(
            path,
            "/user_secure_actions" | "/user_secure_actions/consume"
        )
    {
        return ("No authenticated session required; secure-action tokens and input validation are enforced by the handler.",json!([]));
    }
    ("User session required; resource ownership, grants, blocks and other handler checks still apply. Delegated credentials require an allowlisted route and scope.",json!([{"sessionBearer":[]}]))
}

fn summary(operation: &Operation, path: &str) -> String {
    let name = operation.handler.rsplit("::").next().unwrap_or("operation");
    let name = name.trim_end_matches("_route");
    let mut text = if matches!(
        name,
        "get" | "list" | "create" | "update" | "delete" | "traces" | "generate"
    ) {
        format!(
            "{name} {}",
            path.trim_matches('/').replace('/', " ").replace(':', "")
        )
    } else {
        name.replace('_', " ")
    };
    if let Some(first) = text.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    text
}

fn purpose(method: &str, path: &str, fallback: &str) -> String {
    let precise=match (method,path) {
        ("GET","/me/writing_nudges/current")=>"Select or retrieve one stable, private writing nudge for the caller; returns {nudge:object|null}. Fetching does not record exposure. Independent of feed pagination and AI consent.",
        ("POST","/me/writing_nudges/:nudge_id/events")=>"Record shown, dismissed or opened for an owned writing suggestion. Idempotent 204; stale/unknown IDs are harmless no-ops. Does not change trace content or seen state.",
        ("GET","/admin/analytics/user_segments")=>"Current mutually exclusive user breakdown: churners, private writers, publishers, readers, other; lifetime writing counts and 30-day reading/writing inactivity. Admin only.",
        ("GET","/admin/analytics/writing_nudges")=>"Writing-nudge exposure cohorts by type: shown/opened/dismissed counts, distinct users, no recorded action and opening/dismissal percentages. Date range selects first exposures; outcomes included through as_of. Admin only.",
        ("GET","/admin/analytics/devices")=>"Current recently-active device snapshot: distinct users/devices by platform, app version, OS and browser; multi-platform users and registration coverage. active_within_days defaults to 30 (1–366). Excludes service/demo accounts and revoked devices. Admin only.",
        ("GET","/admin/users/:id/devices")=>"Paginated device metadata for one user, including revoked devices, ordered by last activity. Exposes push-token presence/provider/update time but never token values or installation identifiers. Admin only.",
        ("GET","/users/:id")=>"Read a public user profile, or the caller's private self profile. The self response includes mentor_feedback_style (balanced, challenging, companionable); public profiles do not expose it.",
        ("PATCH","/users/:id")=>"Update the caller's own account fields, including optional mentor_feedback_style (balanced, challenging, companionable). Missing values preserve existing preferences. Does not trigger AI generation.",
        ("PUT","/users/:id")=>"Update the caller's own profile with the existing user payload; optional mentor_feedback_style preserves its current value when omitted.",
        ("GET","/writing_templates")=>"List active platform-managed writing starters; paginated, ordered by sort_order/slug/id, optional exact locale filter. Authenticated.",
        ("POST","/conversations")=>"Create a group or get/create the unique direct conversation with a partner. Groups require accepted relationships with the inviter and compatible membership; history starts at joining.",
        ("GET","/conversations")=>"Paginated active conversations with optional q search over titles and other active members' public display names/handles (case/accent-insensitive, max 200 characters). Ordered by sort_at: newest readable processed message date, or the caller's joined_at when none exists. Includes unread counts, participant previews and membership settings.",
        ("GET","/me/conversation_events")=>"Authenticated best-effort SSE for conversation/message/reaction changes. Starts with resync_required; no persisted event history or Last-Event-ID replay. Message payloads are hydrated for the current caller and enforce membership/history/session access. Keep HTTP writes and background push behavior unchanged; reconnect and resynchronize through normal read APIs.",
        ("GET","/conversations/:id")=>"Read an active conversation and its caller-specific summary. Membership and history boundaries are enforced.",
        ("PATCH","/conversations/:id")=>"Update a group title as an admin and/or mute/unmute notifications for the caller.",
        ("GET","/conversations/:id/members")=>"Paginated current conversation members with public profiles, roles and join dates.",
        ("POST","/conversations/:id/members")=>"Add group members directly as an admin; respects relationships, blocks, sandbox boundaries and since-joining history.",
        ("PATCH","/conversations/:id/members/:user_id")=>"Set/clear a shared conversation nickname as any active member (nickname string/null, max 80 characters). Optional role changes remain group-admin-only and protect the last active admin. Combined changes are atomic; returns 204.",
        ("DELETE","/conversations/:id/members/:user_id")=>"Leave a group or remove another member as an admin; immediately revokes conversation and message access.",
        ("GET","/conversations/:id/messages")=>"Read visible processed messages newest-position-first; use before_position for pagination, not offset.",
        ("POST","/conversations/:id/messages")=>"Send text/replies/trace-post links and up to four image_attachment_ids to a human direct/group conversation; image-only messages may use empty content. Direct chats preserve mentor workflows, but mentor images are not supported. Returns message (including images metadata) and optional pending_reply_message.",
        ("POST","/conversations/:id/images")=>"Upload one temporary JPEG/PNG/WebP through the API (multipart file, 5 MiB maximum). Human conversation members only. Returns image metadata; sending subsequently claims its ID atomically. Remote lifetime is 45 days; no permanent Asset is created.",
        ("GET","/conversations/:id/images/:image_id/url")=>"Get a private download URL valid for at most five minutes and never beyond retention. Requires current access to the image's sent message and its joining-history scope; expired images return 410 after authorization.",
        ("DELETE","/conversations/:id/images/:image_id")=>"Cancel an owned unsent conversation image and delete its storage object. Sent images cannot be cancelled; already-cancelled uploads can be retried. Returns 204.",
        ("PUT","/conversations/:id/read")=>"Advance the caller's read cursor monotonically through a visible conversation position; fetching alone does not mark messages read.",
        ("GET","/writing_templates/:id")=>"Retrieve a writing template by UUID, including inactive starters referenced by existing traces. Authenticated.",
        ("GET","/admin/writing_templates")=>"List active and inactive writing templates; optional locale and is_active filters. Admin only.",
        ("POST","/admin/writing_templates")=>"Create a platform writing template with a stable unique slug, title, description and starter text. Admin only.",
        ("PATCH","/admin/writing_templates/:id")=>"Edit or deactivate a writing template; never rewrites existing traces. Slug is immutable. Admin only.",
        ("GET","/healthz")=>"Public database readiness check: runs SELECT 1 and returns 200 when PostgreSQL responds, otherwise 503. No user data or connection details; responses are not cacheable.",
        ("GET","/api/schema")=>"Discover the live API route catalog in OpenAPI 3.1 format; payload schemas are not fully typed yet.",
        ("GET","/api/docs")=>"Read API conventions or a feature-specific Markdown contract using the topic query parameter.",
        ("POST","/admin/automation/clients")=>"Provision a scoped automation client. The new bearer secret is returned only once.",
        ("POST","/admin/automation/clients/:id/token")=>"Rotate the agent key and revoke its delegated sessions.",
        ("DELETE","/admin/automation/clients/:id")=>"Revoke an automation client; its delegated credentials stop working.",
        ("POST","/automation/demo_users")=>"Atomically create 1–20 sandbox demo users owned by this agent. No normal login credentials or real-user interactions are enabled.",
        ("GET","/automation/demo_users")=>"List this agent's demo accounts with limit/offset pagination.",
        ("GET","/automation/demo_users/:id")=>"Read a demo account owned by this agent.",
        ("PUT","/automation/demo_users/:id")=>"Update this agent's demo profile; ownership, email, password and roles cannot be edited.",
        ("DELETE","/automation/demo_users/:id")=>"Permanently purge this agent's demo account and associated content/assets.",
        ("POST","/automation/demo_users/:id/delegations")=>"Issue a one-hour scoped session acting as this agent's demo user on explicitly allowed platform routes.",
        ("GET","/automation/contract")=>"Read the agent-specific contract, limits and delegated-route allowlist. Documentation never expands permissions.",
        ("GET","/me/capabilities")=>"Inspect the automation credential's effective scopes, sandbox rules and limits.",
        ("GET","/me/audience/:user_id/traces")=>"Audit which of your finalized traces this reader can currently access across journals. Owner only; limit/offset pagination with an accurate total.",
        ("GET","/journals/:id/images")=>"List images from readable finalized journal traces, newest finalized_at first. Lightweight trace/asset IDs and dimensions only; supports limit/offset pagination with a viewer-scoped total.",
        ("GET","/users/:id/publications")=>"List the caller's PUBLISHED channel publications across all owned blogs, including disabled blogs, with blog identity and limit/offset pagination. Owner only; excludes social posts, archived publications and distribution-event history. Sorted by latest distribution date.",
        ("GET","/blog/p/:short_code")=>"Resolve a stable eight-character publication sharing alias to the current canonical blog article URL using a non-cacheable 307 redirect. Rechecks enabled blog, publication/source visibility and demo-account restrictions; unavailable targets return non-cacheable 404. Not an access token or arbitrary redirect URL.",
        ("GET","/blog/_assets/share.js")=>"Public static progressive-enhancement script for article sharing: native share menu, clipboard copy and manual-copy fallback. Contains no user content or tracking.",
        ("GET","/posts/:id/distribution_events")=>"List the post publisher's immutable distribution-event history, newest first, with limit/offset pagination.",
        ("POST","/posts/:id/distribution_events")=>"Record an idempotent post distribution event, update feed position and dispatch to selected eligible recipients. Does not create another durable publication or access grants.",
        ("PUT","/traces/:id/plume")=>"Give one plume to accessible finalized writing by someone else. Idempotent; no request body. Returns plume_count and plumed_by_me.",
        ("DELETE","/traces/:id/plume")=>"Retract your own plume, including after access revocation; inaccessible trace totals are not disclosed.",
        ("GET","/traces/:id/plumes")=>"List people who gave a plume, newest first, with pagination. Trace author only.",
        ("PUT","/traces/:id/post")=>"Prepare/update the caller's trace publication. Configure grants/audiences before publishing. Supports all/none/selected notification targeting; a published-to-published update does not resend publication alerts.",
        ("GET","/posts/:id/notification_settings")=>"Read notification mode and selected recipient IDs; only the publisher can read the selection.",
        ("POST","/traces/:id/messages")=>"Send a trace-linked message. For a mentor question about an owned draft, save first and provide expected_version_integer; the server snapshots draft/thread context. Human messages require an accepted relationship.",
        ("POST","/messages")=>"Send a direct message or mentor request using the selected message type; relationship, AI-consent and context-access checks apply.",
        ("GET","/writing_threads")=>"List the caller's writing threads, newest activity first; journal_id matches any member trace or pending draft in that journal. Supports q and limit/offset pagination. Counts and character totals are persisted.",
        ("GET","/journals/:id/writing_threads")=>"List owned writing threads containing a member trace or pending draft in this journal, including threads spanning multiple journals. Supports q and limit/offset pagination.",
        ("GET","/writing_threads/:id")=>"Read an owned writing thread, generated title/question/journey summary and cached metrics.",
        ("GET","/writing_threads/:id/traces")=>"List finalized, unarchived member traces in continuation order, with limit/offset pagination. Owner only.",
        ("POST","/writing_threads/:id/generation")=>"Queue generation of a writing thread's display information, subject to AI consent and quota.",
        ("GET","/feed")=>"Read accessible feed items with batched active trace mentions (user identity/avatar and allows_reshare); non-trace cards have mentions=[]. Publication notification suppression does not remove items from this feed.",
        ("GET","/posts/feed")=>"Alias of GET /feed, including batched active trace mentions and empty mention lists for other card kinds.",
        ("GET","/me/feed")=>"Read the caller's own feed with batched active trace mentions and seen-by previews. Optional sharing=shared|private filters finalized traces by current exposure and excludes album cards; omission preserves the combined feed. Non-trace cards have mentions=[].",
        ("GET","/me/traces/export")=>"Download owned traces as md, txt or json. Optional journal_ids is a comma-separated list of up to 100 owned journal UUIDs; optional from/to dates are inclusive. Omission exports all journals.",
        ("POST","/me/wal")=>"Append raw input to today's Flux/WAL and reschedule automatic compilation for five minutes after this append. Returns automation.scheduled_for.",
        ("GET","/me/wal")=>"Read today's Flux/WAL, including its persisted compilation schedule, without replacing previous days.",
        ("GET","/me/wals")=>"List the caller's persisted daily Flux/WAL records with pagination.",
        ("POST","/me/wal/compilation")=>"Request operational and thematic WAL projections; AI-consent and usage limits apply.",
        _=>return format!("{fallback}. Consult the linked contracts for request fields, response shapes and domain-specific behavior."),
    };
    precise.to_string()
}

pub async fn schema() -> Json<Value> {
    let routes = registry()
        .lock()
        .expect("API documentation registry poisoned")
        .clone();
    let mut paths = Map::new();
    let mut count = 0;
    for (path, operations) in routes {
        let normalized = openapi_path(&path);
        let parameters = path
            .split('/')
            .filter_map(|part| part.strip_prefix(':'))
            .map(|name| json!({"name":name,"in":"path","required":true,"schema":{"type":"string"}}))
            .collect::<Vec<_>>();
        let mut item = Map::new();
        if !parameters.is_empty() {
            item.insert("parameters".into(), json!(parameters));
        }
        for op in operations {
            count += 1;
            let summary = summary(&op, &path);
            let (auth, security) = access(op.method, &path);
            let scope = crate::entities_v2::platform_infra::automation::documentation_scope(
                op.method, &path,
            );
            let topic =
                if path == "/admin/analytics/devices" || path == "/admin/users/:id/devices" {
                    "devices_analytics"
                } else if path.starts_with("/conversations") || path == "/me/conversation_events" {
                    "conversations"
                } else if path.starts_with("/writing_templates") || path.starts_with("/admin/writing_templates") {
                    "writing_templates"
                } else if path.starts_with("/automation/") || path.starts_with("/admin/automation/") {
                    "automation"
                } else if matches!(path.as_str(), "/me/feed" | "/me/traces/export") {
                    "feed_and_exports"
                } else if path.contains("/distribution_events") || path.contains("/versions") {
                    "post_distribution_events"
                } else if path.starts_with("/blogs") || path.starts_with("/blog/") || path == "/users/:id/publications" {
                    "blogs"
                } else if path.starts_with("/me/writing_nudges/") || path == "/admin/analytics/writing_nudges" {
                    "writing_nudges"
                } else if path.contains("writing_threads") {
                    "writing_threads"
                } else if path.starts_with("/admin/analytics/") || path == "/usage_events" {
                    "analytics_engagement"
                } else if path == "/journals/:id/images" {
                    "journal_images"
                } else if matches!(path.as_str(), "/albums" | "/albums/:id" | "/albums/:id/assets" | "/users/:id/albums" | "/traces/:id/albums") {
                    "album_stats"
                } else if path.contains("/plume") || path.starts_with("/me/audience/") || path.starts_with("/me/wal") {
                    "social_updates"
                } else if path == "/traces/:id/messages" {
                    "draft_mentor"
                } else if path == "/traces/:id/post" || path.ends_with("/notification_settings") {
                    "post_notifications"
                } else {
                    "platform"
                };
            let mut operation = json!({
                "summary":summary,"description":format!("{}\n\n{}",purpose(op.method,&path,&summary),auth),
                "tags":[path.split('/').find(|p|!p.is_empty()).unwrap_or("health")],
                "security":security,
                "responses":{"default":{"description":"Status codes and response body follow the linked feature contract; payload schemas are not typed in this discovery version."}},
                "x-payload-schema-status":"not-yet-typed",
                "x-delegated-agent-scope":scope,
                "x-delegated-agent-allowed":scope.is_some() || matches!(path.as_str(),"/api/schema"|"/api/docs"|"/me/capabilities"),
                "externalDocs":{"url":format!("/api/docs?topic={topic}"),"description":"Detailed feature contract"}
            });
            if path == "/me/conversation_events" {
                operation["responses"]["200"]=json!({"description":"Best-effort SSE stream. Always starts with resync_required; authenticated reconnection and API resynchronization are required, with no event replay.","content":{"text/event-stream":{"schema":{"type":"string"}}}});
            }
            if path.contains("traces") || path.ends_with("/feed") || path == "/feed" {
                operation["x-related-contracts"] = json!(["/api/docs?topic=trace_translations"]);
            }
            if matches!(path.as_str(), "/feed" | "/posts/feed" | "/me/feed") {
                operation["x-related-contracts"] = json!(["/api/docs?topic=trace_translations", "/api/docs?topic=feed_mentions"]);
            }
            if path == "/users/:id" {
                operation["x-related-contracts"] = json!(["/api/docs?topic=mentor_feedback_style"]);
            }
            if matches!(path.as_str(),"/journals/{id}"|"/users/{id}/journals"|"/journals/shared"|"/journals/shared/recent"|"/users/{id}/journals/shared"|"/journals") {
                operation["x-related-contracts"]=json!(["/api/docs?topic=journal_stats"]);
            }
            if path == "/api/docs" {
                operation["parameters"] = json!([{"name":"topic","in":"query","required":false,
                    "description":"Documentation topic; omission returns the overview.",
                    "schema":{"type":"string","enum":["overview","platform","automation","writing_threads","draft_mentor","post_notifications","trace_translations","trace_languages","social_updates","trace_chronology","blogs","post_distribution_events","journal_stats","journal_images","album_stats","feed_and_exports","analytics_engagement","writing_nudges","writing_templates","conversations","devices_analytics","mentor_feedback_style","feed_mentions"]}}]);
            }
            item.insert(op.method.to_ascii_lowercase(), operation);
        }
        paths.insert(normalized, Value::Object(item));
    }
    Json(
        json!({"openapi":"3.1.0","info":{"title":"Hupo API","version":env!("CARGO_PKG_VERSION"),
        "description":"Route-first API discovery generated from actual route registration. Every explicit HTTP operation is listed; request/response payload schemas are not yet typed. Not a complete SDK-generation contract. Resource permissions and agent scopes remain enforced independently."},
        "servers":[{"url":"/"}],"paths":paths,
        "components":{"securitySchemes":{
            "sessionBearer":{"type":"http","scheme":"bearer","bearerFormat":"session_id.secret","description":"Regular or delegated session token; not a JWT."},
            "agentBearer":{"type":"http","scheme":"bearer","bearerFormat":"agent.client_id.secret"},
            "internalCronToken":{"type":"apiKey","in":"header","name":"x-internal-cron-token"},
            "journalShareToken":{"type":"apiKey","in":"query","name":"token"}
        }},
        "x-coverage":{"routes":"all-explicit-registrations","operation_count":count,"payloads":"not-yet-typed",
            "descriptions":"handler-derived-with-curated-overrides","excluded":["implicit HEAD/OPTIONS","static /public files","404 fallback"]},
        "x-documentation-topics":["overview","platform","automation","writing_threads","draft_mentor","post_notifications","trace_translations","trace_languages","social_updates","trace_chronology","blogs","post_distribution_events","journal_stats","journal_images","album_stats","feed_and_exports","analytics_engagement","writing_nudges","writing_templates","conversations","devices_analytics","mentor_feedback_style","feed_mentions"],
        "externalDocs":{"url":"/api/docs","description":"Usage guide and detailed feature contracts"}}),
    )
}

#[derive(Deserialize)]
pub struct DocsQuery {
    pub topic: Option<String>,
}
pub async fn docs(Query(query): Query<DocsQuery>) -> Response {
    let text = match query.topic.as_deref().unwrap_or("overview") {
        "overview" => include_str!("../doc/api_discovery.md"),
        "platform" => include_str!("../doc/api_quick.md"),
        "automation" => include_str!("../doc/automation_api.md"),
        "writing_threads" => include_str!("../doc/writing_threads.md"),
        "writing_nudges" => include_str!("../doc/writing_nudges.md"),
        "writing_templates" => include_str!("../doc/writing_templates.md"),
        "conversations" => include_str!("../doc/conversations.md"),
        "devices_analytics" => include_str!("../doc/devices_analytics.md"),
        "mentor_feedback_style" => include_str!("../doc/mentor_feedback_style.md"),
        "feed_mentions" => include_str!("../doc/feed_mentions.md"),
        "draft_mentor" => include_str!("../doc/draft_mentor_messages.md"),
        "post_notifications" => include_str!("../doc/post_notifications.md"),
        "trace_translations" => include_str!("../doc/trace_translation_messages.md"),
        "trace_languages" => include_str!("../doc/trace_languages.md"),
        "trace_chronology" => include_str!("../doc/trace_chronology.md"),
        "blogs" => include_str!("../doc/blogs.md"),
        "post_distribution_events" => include_str!("../doc/post_distribution_events.md"),
        "journal_stats" => include_str!("../doc/journal_stats.md"),
        "journal_images" => include_str!("../doc/journal_images.md"),
        "album_stats" => include_str!("../doc/album_stats.md"),
        "feed_and_exports" => include_str!("../doc/feed_and_export_filters.md"),
        "analytics_engagement" => include_str!("../doc/analytics_engagement.md"),
        "social_updates" => include_str!("../doc/social_and_wal_updates.md"),
        _ => return (StatusCode::NOT_FOUND, "Unknown documentation topic").into_response(),
    };
    (
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        text,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn schema_tracks_registered_routes_and_is_explicit_about_payload_coverage() {
        // Construct only; no database, server, provider calls or workers are started.
        std::env::set_var("ALLOW_ORIGIN", "^http://localhost$");
        let _router = crate::router::create_router();
        let _again = crate::router::create_router();
        let document = schema().await.0;
        assert_eq!(document["openapi"], "3.1.0");
        assert_eq!(document["paths"]["/healthz"]["get"]["security"], json!([]));
        assert_eq!(document["x-coverage"]["payloads"], "not-yet-typed");
        let paths = document["paths"].as_object().unwrap();
        assert!(paths.len() > 100);
        assert!(paths["/users"].get("get").is_some());
        assert!(paths["/users"].get("post").is_some());
        assert!(paths["/me/writing_nudges/current"].get("get").is_some());
        assert!(paths["/me/writing_nudges/{nudge_id}/events"].get("post").is_some());
        assert_eq!(
            paths["/me/writing_nudges/current"]["get"]["externalDocs"]["url"],
            "/api/docs?topic=writing_nudges"
        );
        assert!(paths["/admin/analytics/users"].get("get").is_some());
        assert!(paths["/admin/analytics/user_segments"].get("get").is_some());
        assert!(paths["/admin/analytics/writing_nudges"].get("get").is_some());
        for path in ["/admin/analytics/devices", "/admin/users/{id}/devices"] {
            assert_eq!(paths[path]["get"]["externalDocs"]["url"], "/api/docs?topic=devices_analytics");
            assert!(paths[path]["get"]["description"].as_str().unwrap().contains("Admin only"));
        }
        assert!(paths["/writing_templates"].get("get").is_some());
        assert!(paths["/writing_templates/{id}"].get("get").is_some());
        assert!(paths["/admin/writing_templates"].get("post").is_some());
        assert!(paths["/admin/writing_templates/{id}"].get("patch").is_some());
        assert!(paths["/conversations"].get("post").is_some());
        assert_eq!(paths["/me/conversation_events"]["get"]["externalDocs"]["url"], "/api/docs?topic=conversations");
        assert!(paths["/conversations/{id}/messages"].get("get").is_some());
        assert!(paths["/conversations/{id}/members/{user_id}"].get("delete").is_some());
        assert!(paths["/conversations/{id}/read"].get("put").is_some());
        assert_eq!(
            paths["/users/{id}/publications"]["get"]["externalDocs"]["url"],
            "/api/docs?topic=blogs"
        );
        assert!(paths["/posts/{id}/distribution_events"].get("get").is_some());
        assert!(paths["/posts/{id}/distribution_events"].get("post").is_some());
        assert!(paths["/blogs/{id}/publications"].get("get").is_some());
        assert!(!paths.contains_key("/users/{id}/blog_posts"));
        assert!(!paths.contains_key("/posts/{id}/publications"));
        assert!(!paths.contains_key("/blogs/{id}/posts"));
        assert_eq!(
            docs(Query(DocsQuery { topic: Some("post_distribution_events".into()) })).await.status(),
            StatusCode::OK
        );
        assert_eq!(
            docs(Query(DocsQuery { topic: Some("post_publications".into()) })).await.status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            paths["/journals/{id}/images"]["get"]["externalDocs"]["url"],
            "/api/docs?topic=journal_images"
        );
        assert!(paths["/internal/generate_shared_journal_daily_digests"]
            .get("post")
            .is_some());
        assert_eq!(
            paths["/internal/run_pending_analyses"]["post"]["security"],
            json!([{"internalCronToken":[]}])
        );
        assert_eq!(paths["/api/schema"]["get"]["security"], json!([]));
        assert_eq!(
            paths["/traces/{id}"]["patch"]["x-delegated-agent-scope"],
            "content:write"
        );
        assert_eq!(
            paths["/admin/automation/clients"]["post"]["x-delegated-agent-allowed"],
            false
        );
        assert_eq!(paths["/api/docs"]["get"]["x-delegated-agent-allowed"], true);
        let mut count = 0;
        for (path, item) in paths {
            assert!(!path.contains(':'));
            for verb in ["get", "post", "put", "patch", "delete"] {
                if let Some(operation) = item.get(verb) {
                    count += 1;
                    assert!(!operation["description"].as_str().unwrap().is_empty());
                    assert!(operation["responses"]["default"]
                        .get("description")
                        .is_some());
                }
            }
        }
        assert_eq!(document["x-coverage"]["operation_count"], count);
        assert_eq!(
            docs(Query(DocsQuery { topic: None })).await.status(),
            StatusCode::OK
        );
        assert_eq!(
            docs(Query(DocsQuery {
                topic: Some("../secret".into())
            }))
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
}
