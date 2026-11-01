use std::sync::OnceLock;
use std::time::Instant;

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::{header::HeaderName, HeaderMap, HeaderValue, Request, Response};
use axum::middleware::Next;
use tracing::Instrument;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::entities_v2::error::{PpdcErrorLogContext, PpdcRequestLogContext};

static TRACING_GUARDS: OnceLock<Vec<WorkerGuard>> = OnceLock::new();

pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Response-local metadata, never serialized or returned as HTTP headers.
#[derive(Clone)]
pub(crate) struct AuthenticationLogContext {
    pub user_id: String,
    pub session_id: String,
    pub auth_ms: u64,
}

struct RequestCompletion<'a> {
    request_id: &'a str,
    method: &'a str,
    route: &'a str,
    started_at: Instant,
    finished: bool,
}

impl Drop for RequestCompletion<'_> {
    fn drop(&mut self) {
        if !self.finished {
            // A dropped future or panic is not proof that the client disconnected.
            tracing::warn!(
                target: "api",
                request_id = self.request_id,
                http_method = self.method,
                http_route = self.route,
                latency_ms = self.started_at.elapsed().as_millis() as u64,
                panicking = std::thread::panicking(),
                "http_request_aborted"
            );
        }
    }
}

fn request_id(headers: &HeaderMap) -> String {
    headers
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= 128 && value.parse::<uuid::Uuid>().is_ok())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

pub async fn request_context_middleware(request: Request<Body>, next: Next) -> Response<Body> {
    let request_id = request_id(request.headers());
    let method = request.method().to_string();
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or_else(|| request.uri().path())
        .to_string();
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        http_method = %method,
        http_route = %route,
        user_id = tracing::field::Empty,
        session_id = tracing::field::Empty,
        auth_ms = tracing::field::Empty,
    );

    async move {
        let mut completion = RequestCompletion {
            request_id: &request_id,
            method: &method,
            route: &route,
            started_at: Instant::now(),
            finished: false,
        };
        tracing::info!(target: "api", "http_request_started");
        let mut response = next.run(request).await;
        // Includes authentication and response construction, not body delivery.
        let latency_ms = completion.started_at.elapsed().as_millis();
        let status_code = response.status().as_u16();
        let auth = response
            .extensions_mut()
            .remove::<AuthenticationLogContext>();
        let user_id = auth.as_ref().map(|a| a.user_id.as_str()).unwrap_or("");
        let session_id = auth.as_ref().map(|a| a.session_id.as_str()).unwrap_or("");
        let auth_ms = auth.as_ref().map(|a| a.auth_ms).unwrap_or_default();

        if let Ok(value) = HeaderValue::from_str(&request_id) {
            response
                .headers_mut()
                .insert(HeaderName::from_static(REQUEST_ID_HEADER), value);
        }

        if let Some(error) = response.extensions_mut().remove::<PpdcErrorLogContext>() {
            error.emit(
                Some(&PpdcRequestLogContext {
                    request_id: &request_id,
                    method: &method,
                    route: &route,
                    user_id,
                    session_id,
                    latency_ms,
                }),
                "api_request_failed",
            );
        } else {
            tracing::info!(
                target: "api",
                request_id,
                http_method = method,
                http_route = route,
                user_id,
                session_id,
                status_code,
                latency_ms = latency_ms as u64,
                auth_ms,
                timing_scope = "request_to_response_headers",
                "http_request_completed"
            );
        }

        completion.finished = true;
        response
    }
    .instrument(span)
    .await
}

fn is_work_analyzer_target(target: &str) -> bool {
    target == "work_analyzer" || target.starts_with("web_server::work_analyzer")
}

fn is_app_target(target: &str) -> bool {
    !is_work_analyzer_target(target)
}

fn build_json_layer<S, W>(
    writer: W,
) -> tracing_subscriber::fmt::Layer<
    S,
    tracing_subscriber::fmt::format::JsonFields,
    tracing_subscriber::fmt::format::Format<tracing_subscriber::fmt::format::Json>,
    W,
>
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    W: for<'writer> tracing_subscriber::fmt::MakeWriter<'writer> + Send + Sync + 'static,
{
    fmt::layer()
        .json()
        .with_writer(writer)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_current_span(false)
        .with_span_list(true)
        .flatten_event(true)
}

pub fn init_tracing() {
    let _ = std::fs::create_dir_all("logs");

    let work_analyzer_file_appender =
        tracing_appender::rolling::never("logs", "work_analyzer.jsonl");
    let (work_analyzer_non_blocking, work_analyzer_guard) =
        tracing_appender::non_blocking(work_analyzer_file_appender);

    let app_file_appender = tracing_appender::rolling::never("logs", "app.jsonl");
    let (app_non_blocking, app_guard) = tracing_appender::non_blocking(app_file_appender);

    let _ = TRACING_GUARDS.set(vec![work_analyzer_guard, app_guard]);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = build_json_layer(std::io::stdout);

    let work_analyzer_file_layer =
        build_json_layer(work_analyzer_non_blocking).with_filter(filter_fn(|metadata| {
            is_work_analyzer_target(metadata.target())
        }));

    let app_file_layer = build_json_layer(app_non_blocking)
        .with_filter(filter_fn(|metadata| is_app_target(metadata.target())));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(work_analyzer_file_layer)
        .with(app_file_layer)
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        sync::{Arc, Mutex},
        time::Duration,
    };

    #[derive(Clone, Default)]
    struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

    impl Write for CapturedLogs {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn preserves_valid_incoming_request_id() {
        let id = uuid::Uuid::new_v4();
        let mut headers = HeaderMap::new();
        headers.insert(
            REQUEST_ID_HEADER,
            HeaderValue::from_str(&id.to_string()).unwrap(),
        );

        assert_eq!(request_id(&headers), id.to_string());
    }

    #[test]
    fn replaces_invalid_incoming_request_id() {
        let mut headers = HeaderMap::new();
        headers.insert(REQUEST_ID_HEADER, HeaderValue::from_static("not-a-uuid"));

        assert!(request_id(&headers).parse::<uuid::Uuid>().is_ok());
    }

    #[test]
    fn optional_ids_and_nginx_generated_ids_are_supported() {
        let generated = request_id(&HeaderMap::new());
        assert!(generated.parse::<uuid::Uuid>().is_ok());
        let nginx_id = uuid::Uuid::new_v4().simple().to_string();
        let mut headers = HeaderMap::new();
        headers.insert(REQUEST_ID_HEADER, HeaderValue::from_str(&nginx_id).unwrap());
        assert_eq!(request_id(&headers), nginx_id);
    }

    #[tokio::test]
    async fn request_timing_covers_outer_work_and_keeps_auth_and_error_behavior() {
        use axum::{extract::Extension, middleware::from_fn, routing::get, Router};
        use diesel::{r2d2::ConnectionManager, PgConnection};
        use tracing::instrument::WithSubscriber;

        // No database connection is opened: anonymous session resolution only
        // needs the pool extension, and min_idle=0 keeps this test self-contained.
        let pool = crate::db::DbPool::builder()
            .max_size(1)
            .min_idle(Some(0))
            .build(ConnectionManager::<PgConnection>::new(
                "postgres://unused/unused",
            ))
            .unwrap();
        let app = Router::new()
            .route("/ok", get(|| async { "ok" }))
            .route(
                "/private",
                get(|| async { "private" })
                    .layer(from_fn(crate::sessions_service::auth_middleware_custom)),
            )
            .route(
                "/error",
                get(|| async { crate::entities_v2::error::PpdcError::unauthorized() }),
            )
            .layer(from_fn(crate::sessions_service::add_session_to_request))
            .layer(from_fn(|req: Request<Body>, next: Next| async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                next.run(req).await
            }))
            .layer(from_fn(request_context_middleware))
            .layer(Extension(pool));
        let captured = CapturedLogs::default();
        let writer = captured.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_ansi(false)
            .with_current_span(false)
            .with_span_list(true)
            .flatten_event(true)
            .with_writer(move || writer.clone())
            .finish();
        // Axum spawns connection tasks; attach the test subscriber per request
        // rather than relying on it being inherited by spawned tasks.
        let dispatch = tracing::Dispatch::new(subscriber);
        let app = app.layer(from_fn(move |req: Request<Body>, next: Next| {
            next.run(req).with_subscriber(dispatch.clone())
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let response = client
            .get(format!("{base}/ok"))
            .header(REQUEST_ID_HEADER, &id)
            .send()
            .await
            .unwrap();
        assert_eq!(response.headers()[REQUEST_ID_HEADER], id);
        assert_eq!(response.text().await.unwrap(), "ok");
        let response = client.get(format!("{base}/private")).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        assert!(response.headers()[REQUEST_ID_HEADER]
            .to_str()
            .unwrap()
            .parse::<uuid::Uuid>()
            .is_ok());
        let response = client
            .get(format!("{base}/error"))
            .header(REQUEST_ID_HEADER, "invalid")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
        let error_id = response.headers()[REQUEST_ID_HEADER]
            .to_str()
            .unwrap()
            .to_owned();
        assert!(error_id.parse::<uuid::Uuid>().is_ok());
        server.abort();
        let _ = server.await;
        let logs = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        let events = logs
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        let completed = events
            .iter()
            .find(|e| e["message"] == "http_request_completed" && e["request_id"] == id)
            .unwrap();
        assert!(completed["latency_ms"].as_u64().unwrap() >= 20);
        assert!(completed["auth_ms"].is_number());
        assert!(completed["session_id"]
            .as_str()
            .unwrap()
            .parse::<uuid::Uuid>()
            .is_ok());
        assert!(events
            .iter()
            .any(|e| e["message"] == "http_request_started"));
        let error = events
            .iter()
            .find(|e| e["message"] == "api_request_failed" && e["request_id"] == error_id)
            .unwrap();
        assert!(error["session_id"]
            .as_str()
            .unwrap()
            .parse::<uuid::Uuid>()
            .is_ok());
        assert!(!events
            .iter()
            .any(|e| e["message"] == "http_request_aborted"));
    }
}
