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
    if path.starts_with("/shared/") {
        return (
            "Public journal-sharing token, validated by the handler.",
            json!([{"journalShareToken":[]}]),
        );
    }
    if matches!(
        path,
        "/" | "/api/schema" | "/api/docs" | "/mobile/version_policy"
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
        ("PUT","/traces/:id/post")=>"Prepare/update the caller's trace publication. Configure grants/audiences before publishing. Supports all/none/selected notification targeting; a published-to-published update does not resend publication alerts.",
        ("GET","/posts/:id/notification_settings")=>"Read notification mode and selected recipient IDs; only the publisher can read the selection.",
        ("POST","/traces/:id/messages")=>"Send a trace-linked message. For a mentor question about an owned draft, save first and provide expected_version_integer; the server snapshots draft/thread context. Human messages require an accepted relationship.",
        ("POST","/messages")=>"Send a direct message or mentor request using the selected message type; relationship, AI-consent and context-access checks apply.",
        ("GET","/writing_threads")=>"List the caller's writing threads, newest activity first; supports journal_id, q and limit/offset pagination. Counts and character totals are persisted.",
        ("GET","/writing_threads/:id")=>"Read an owned writing thread, generated title/question/journey summary and cached metrics.",
        ("GET","/writing_threads/:id/traces")=>"List finalized, unarchived member traces in continuation order, with limit/offset pagination. Owner only.",
        ("POST","/writing_threads/:id/generation")=>"Queue generation of a writing thread's display information, subject to AI consent and quota.",
        ("GET","/feed")=>"Read accessible feed items; publication notification suppression does not remove items from this feed.",
        ("GET","/me/feed")=>"Read the caller's own feed, including batched seen-by previews for trace cards.",
        ("POST","/me/wal")=>"Append raw input to today's Flux/WAL; new days are created lazily and do not overwrite previous days.",
        ("GET","/me/wal")=>"Read today's Flux/WAL without replacing previous days.",
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
                if path.starts_with("/automation/") || path.starts_with("/admin/automation/") {
                    "automation"
                } else if path.contains("writing_threads") {
                    "writing_threads"
                } else if path == "/traces/:id/messages" {
                    "draft_mentor"
                } else if path == "/traces/:id/post" || path.ends_with("/notification_settings") {
                    "publication_notifications"
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
            if path.contains("traces") || path.ends_with("/feed") || path == "/feed" {
                operation["x-related-contracts"] = json!(["/api/docs?topic=trace_translations"]);
            }
            if path == "/api/docs" {
                operation["parameters"] = json!([{"name":"topic","in":"query","required":false,
                    "description":"Documentation topic; omission returns the overview.",
                    "schema":{"type":"string","enum":["overview","platform","automation","writing_threads","draft_mentor","publication_notifications","trace_translations","trace_languages"]}}]);
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
        "x-documentation-topics":["overview","platform","automation","writing_threads","draft_mentor","publication_notifications","trace_translations","trace_languages"],
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
        "draft_mentor" => include_str!("../doc/draft_mentor_messages.md"),
        "publication_notifications" => include_str!("../doc/publication_notifications.md"),
        "trace_translations" => include_str!("../doc/trace_translation_messages.md"),
        "trace_languages" => include_str!("../doc/trace_languages.md"),
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
        assert_eq!(document["x-coverage"]["payloads"], "not-yet-typed");
        let paths = document["paths"].as_object().unwrap();
        assert!(paths.len() > 100);
        assert!(paths["/users"].get("get").is_some());
        assert!(paths["/users"].get("post").is_some());
        assert!(paths["/admin/analytics/users"].get("get").is_some());
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
