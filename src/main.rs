use web_server;
use diesel::pg::PgConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use web_server::db;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
use diesel::r2d2::{self, ConnectionManager};
use axum::Extension;
use web_server::environment::get_database_url;

type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

#[tokio::main]
async fn main() {


    //let mut connection = db::establish_connection();
    //connection.run_pending_migrations(MIGRATIONS).expect("should run migrations if any");
    let database_url = get_database_url();
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(15)
        .build(manager)
        .expect("Failed to create pool.");

    let app = web_server::router::create_router()
        .layer(Extension(pool));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
