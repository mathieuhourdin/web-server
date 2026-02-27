pub mod hydrate;
pub mod model;
pub mod persist;
pub mod routes;

pub use model::{NewPost, NewPostDto, Post, PostType};
pub use routes::{
    get_post_route, get_posts_route, get_user_posts_route, post_post_route, put_post_route,
};
