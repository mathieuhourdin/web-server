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

use crate::entities_v2::{
    error::{PpdcErrorLogContext, PpdcRequestLogContext},
    session::Session,
};

static TRACING_GUARDS: OnceLock<Vec<WorkerGuard>> = OnceLock::new();

pub const REQUEST_ID_HEADER: &str = "x-request-id";

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
    let session = request.extensions().get::<Session>();
    let user_id = session
        .and_then(|session| session.user_id)
        .map(|id| id.to_string())
        .unwrap_or_default();
    let session_id = session
        .map(|session| session.id.to_string())
        .unwrap_or_default();
    let span = tracing::info_span!(
        "http_request",
        request_id = %request_id,
        http_method = %method,
        http_route = %route,
        user_id = %user_id,
        session_id = %session_id,
    );

    async move {
        let started_at = Instant::now();
        let mut response = next.run(request).await;
        let latency_ms = started_at.elapsed().as_millis();
        let status_code = response.status().as_u16();

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
                    user_id: &user_id,
                    session_id: &session_id,
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
                latency_ms,
                "http_request_completed"
            );
        }

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
}
