use web_server;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use web_server::db;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use axum::Extension;

#[tokio::main]
async fn main() {

    let pool = db::create_pool();

    let mut conn = pool.get().expect("Failed to get a connection from the pool");
    conn.run_pending_migrations(MIGRATIONS).expect("should run migrations if any");


    let app = web_server::router::create_router()
        .layer(Extension(pool));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
