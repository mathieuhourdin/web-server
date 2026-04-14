use std::sync::OnceLock;

use tracing_subscriber::filter::filter_fn;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static TRACING_GUARDS: OnceLock<Vec<WorkerGuard>> = OnceLock::new();

fn is_work_analyzer_target(target: &str) -> bool {
    target == "work_analyzer" || target.starts_with("web_server::work_analyzer")
}

fn is_api_target(target: &str) -> bool {
    (target == "api"
        || target.starts_with("web_server::entities_v2")
        || target.starts_with("web_server::openai_handler")
        || target.starts_with("web_server::router")
        || target.starts_with("web_server::environment")
        || target.starts_with("web_server::db"))
        && !is_work_analyzer_target(target)
}

pub fn init_tracing() {
    let _ = std::fs::create_dir_all("logs");

    let work_analyzer_file_appender =
        tracing_appender::rolling::never("logs", "work_analyzer.log");
    let (work_analyzer_non_blocking, work_analyzer_guard) =
        tracing_appender::non_blocking(work_analyzer_file_appender);

    let api_file_appender = tracing_appender::rolling::never("logs", "api.log");
    let (api_non_blocking, api_guard) = tracing_appender::non_blocking(api_file_appender);

    let _ = TRACING_GUARDS.set(vec![work_analyzer_guard, api_guard]);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(false)
        .with_target(true)
        .with_level(true);

    let work_analyzer_file_layer = fmt::layer()
        .with_writer(work_analyzer_non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_filter(filter_fn(|metadata| is_work_analyzer_target(metadata.target())));

    let api_file_layer = fmt::layer()
        .with_writer(api_non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_filter(filter_fn(|metadata| is_api_target(metadata.target())));

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(work_analyzer_file_layer)
        .with(api_file_layer)
        .try_init();
}
