use crate::environment::get_database_url;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use std::{sync::OnceLock, time::Duration};

pub type DbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

static GLOBAL_POOL: OnceLock<DbPool> = OnceLock::new();

#[derive(Debug)]
struct PoolTimingEvents;

impl r2d2::HandleEvent for PoolTimingEvents {
    fn handle_checkout(&self, event: r2d2::event::CheckoutEvent) {
        if event.duration() >= Duration::from_millis(100) {
            tracing::warn!(
                target: "db_pool",
                connection_id = event.connection_id(),
                checkout_ms = event.duration().as_millis() as u64,
                "db_pool_checkout_slow"
            );
        }
    }

    fn handle_timeout(&self, event: r2d2::event::TimeoutEvent) {
        tracing::warn!(
            target: "db_pool",
            timeout_ms = event.timeout().as_millis() as u64,
            "db_pool_checkout_timeout"
        );
    }

    fn handle_checkin(&self, event: r2d2::event::CheckinEvent) {
        if event.duration() >= Duration::from_secs(5) {
            tracing::warn!(
                target: "db_pool",
                connection_id = event.connection_id(),
                held_ms = event.duration().as_millis() as u64,
                "db_pool_connection_held_long"
            );
        }
    }
}

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
        .event_handler(Box::new(PoolTimingEvents))
        .build(manager)
        .expect("Failed to create pool.");
    pool
}

pub fn establish_connection() -> PgConnection {
    let database_url = get_database_url();
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
