//! Public publication channels. Social CRUD is isolated through `social_posts`.
mod model;
mod render;
mod routes;

pub use routes::*;

#[cfg(test)]
mod tests;
