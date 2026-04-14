use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use web_server;
use web_server::db;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use axum::Extension;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    web_server::logging::init_tracing();
    let pool = db::create_pool();
    db::init_global_pool(pool.clone());

    let mut conn = pool.get()?;
    conn.run_pending_migrations(MIGRATIONS)?;

    let app = web_server::router::create_router().layer(Extension(pool.clone()));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}
