use std::net::{TcpListener};
use web_server::threadpool::ThreadPool;
use web_server;
use tokio::runtime::Runtime;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use web_server::db;
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").unwrap();
    let pool = ThreadPool::new(4);

    let mut connection = db::establish_connection();
    connection.run_pending_migrations(MIGRATIONS).expect("should run migrations if any");

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        pool.execute(|| {
            let rt = Runtime::new().unwrap();
            rt.block_on(async {
                web_server::handle_connection(stream).await;
            })
        })
    }
}
