use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, Router},
    extract::{Query, Json},
    debug_handler,
};

use serde::{Serialize, Deserialize};
use std::net::SocketAddr;
use tokio::runtime::Runtime;

use web_server;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use web_server::db;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {

    let app = Router::new()
        .route("/", get(root_route));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    let mut connection = db::establish_connection();
    connection.run_pending_migrations(MIGRATIONS).expect("should run migrations if any");
}

// Handler for the root path
async fn root_route() -> &'static str {
    println!("Error");
    "Welcome to the Axum server!"
}
