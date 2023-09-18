use diesel::pg::PgConnection;
use diesel::prelude::*;
use crate::environment::get_database_url;

pub fn establish_connection() -> PgConnection {
    let database_url = get_database_url();
    PgConnection::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
