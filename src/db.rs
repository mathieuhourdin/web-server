use crate::environment::get_database_url;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use std::sync::OnceLock;

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

static GLOBAL_POOL: OnceLock<DbPool> = OnceLock::new();

pub fn init_global_pool(pool: DbPool) {
    GLOBAL_POOL.set(pool).expect("Pool already initialized");
}

pub fn get_global_pool() -> &'static DbPool {
    GLOBAL_POOL.get().expect("Global pool not initialized")
}

pub fn create_pool() -> DbPool {
    let database_url = get_database_url();
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .max_size(15)
        .build(manager)
        .expect("Failed to create pool.");
    pool
}

pub fn establish_connection() -> PgConnection {
    let database_url = get_database_url();
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
