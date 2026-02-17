use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

static TRACING_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

pub fn init_tracing() {
    let _ = std::fs::create_dir_all("logs");

    let file_appender = tracing_appender::rolling::never("logs", "work_analyzer.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = TRACING_GUARD.set(guard);

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("work_analyzer=info"));

    let fmt_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(false)
        .with_level(false)
        .without_time();

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .try_init();
}
