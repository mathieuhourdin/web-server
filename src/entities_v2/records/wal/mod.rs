pub mod model;
pub mod routes;

pub use routes::{
    get_wal_compilation_route, get_wal_route, post_wal_compilation_route, post_wal_route,
    put_wal_route,
};
