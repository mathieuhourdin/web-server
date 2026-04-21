use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static TRACING_GUARDS: OnceLock<Vec<WorkerGuard>> = OnceLock::new();

fn is_work_analyzer_target(target: &str) -> bool {
    target == "work_analyzer" || target.starts_with("web_server::work_analyzer")
}

fn is_app_target(target: &str) -> bool {
    !is_work_analyzer_target(target)
}

fn build_json_layer<S, W>(writer: W) -> tracing_subscriber::fmt::Layer<S, tracing_subscriber::fmt::format::JsonFields, tracing_subscriber::fmt::format::Format<tracing_subscriber::fmt::format::Json>, W>
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

    let work_analyzer_file_layer = build_json_layer(work_analyzer_non_blocking).with_filter(
        filter_fn(|metadata| is_work_analyzer_target(metadata.target())),
    );

    let app_file_layer =
        build_json_layer(app_non_blocking).with_filter(filter_fn(|metadata| {
            is_app_target(metadata.target())
        }));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(work_analyzer_file_layer)
        .with(app_file_layer)
        .try_init();
}
