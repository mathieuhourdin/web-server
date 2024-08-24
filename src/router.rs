use axum::{
    routing::{get, put, Router},
    middleware::from_fn,
    http::Method,
};
use tower_http::cors::{CorsLayer, Any};

use crate::http::{HttpRequest, HttpResponse, StatusCode};
use crate::entities::{user::self,
    interaction::{interaction_routes as interaction},
    error::PpdcError,
    comment,
    resource
};
use crate::sessions_service;
use crate::file_converter;
use crate::entities::resource_relation;
use crate::entities::category;
use crate::link_preview;


pub fn create_router() -> Router {

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(vec![Method::GET, Method::POST, Method::PUT])
        .allow_headers(Any);

    let users_router = Router::new()
        .route("/", get(user::get_users).post(user::post_user))
        .route("/:id", get(user::get_user_route).put(user::put_user_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let resources_router = Router::new()
        .route("/", get(resource::get_resources_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    let interactions_router = Router::new()
        .route("/", get(interaction::get_interactions_route))
        .route("/:id", put(interaction::put_interaction_route))
        .layer(from_fn(sessions_service::auth_middleware_custom));

    Router::new()
        .nest("/users", users_router)
        .nest("/resources", resources_router)
        .nest("/interactions", interactions_router)
        .layer(from_fn(sessions_service::add_session_to_request))
        .layer(cors)
}

pub async fn route_request(request: &mut HttpRequest) -> Result<HttpResponse, PpdcError> {
    let session_id = &request.session.as_ref().unwrap().id;
    println!("Request with session id : {session_id}");
    match (&request.method[..], &request.parsed_path()[..]) {
        ("GET", [""]) => HttpResponse::from_file(StatusCode::Ok, "hello.html"),
        //("GET", ["users", id]) => user::get_user(id, &request),
        //("PUT", ["users", id]) => user::put_user_route(id, &request),
        //("GET", ["users"]) => user::get_users(&request),
        //("POST", ["users"]) => user::post_user(&request),
        //("GET", ["users", id, "thought_inputs"]) => thought_input::get_thought_inputs_for_user(id, &request),
        //("GET", ["users", id, "thought_outputs"]) => thought_output::get_thought_outputs_for_user(id, &request),
        //("GET", ["login"]) => HttpResponse::from_file(StatusCode::Ok, "login.html"),
        ("POST", ["sessions"]) => sessions_service::post_session_route(request).await,
        ("GET", ["sessions"]) => sessions_service::get_session_route(request).await,
        ("PUT", ["comments", id]) => comment::put_comment(id, &request),
        ("GET", ["resources", id, "comments"]) => comment::get_comments_for_resource(id, &request),
        ("POST", ["resources", id, "comments"]) => comment::post_comment_route(id, &request),
        //("GET", ["resources"]) => resource::get_resources_route(&request),
        ("GET", ["resources", id]) => resource::get_resource_route(id, &request),
        ("GET", ["resources", id, "author_interaction"]) => resource::get_resource_author_interaction_route(id, &request),
        ("POST", ["resources", id, "interactions"]) => interaction::post_interaction_for_resource(id, &request),
        ("GET", ["resources", id, "interactions"]) => interaction::get_interactions_for_resource_route(id, &request),
        ("GET", ["resource", id, "bibliography"]) => resource_relation::get_resource_relations_for_resource_route(id, &request),
        ("GET", ["resource", id, "usages"]) => resource_relation::get_targets_for_resource_route(id, &request),
        ("PUT", ["resources", id]) => resource::put_resource_route(id, &request),
        ("POST", ["resources"]) => resource::post_resource_route(&request),
        //("GET", ["interactions"]) => interaction::get_interactions(&request),
        //("PUT", ["interactions", id]) => interaction::put_interaction_route(id, &request),
        ("GET", ["categories"]) => category::get_categories_route(&request),
        ("POST", ["categories"]) => category::post_category_route(&request),
        //("GET", ["thought_outputs"]) => thought_output::get_thought_outputs_route(&request, "all"),
        //("GET", ["thought_outputs", uuid]) => thought_output::get_thought_output_route(uuid),
        //("POST", ["thought_outputs"]) => thought_output::post_thought_outputs_route(&request),
        //("PUT", ["thought_outputs", uuid]) => thought_output::put_thought_output_route(uuid, &request),
        ("POST", ["thought_input_usages"]) => resource_relation::post_resource_relation_route(&request),
        //("POST", ["thought_inputs"]) => thought_input::post_thought_input_route(&request),
        //("PUT", ["thought_inputs", id]) => thought_input::put_thought_input_route(id, &request),
        //("GET", ["thought_inputs", id]) => thought_input::get_one_thought_input_route(id, &request),
        //("GET", ["thought_inputs"]) => thought_input::get_thought_inputs(&request),
        ("GET", ["public", file_name]) => HttpResponse::from_file(StatusCode::Ok, file_name),
        ("POST", ["link_preview"]) => link_preview::post_preview_route(&request).await,
        ("GET", ["list-article"]) => HttpResponse::from_file(StatusCode::Ok, "list-article.html"),
        ("POST", ["file_conversion"]) => file_converter::post_file_conversion_route(&request),
        _ => HttpResponse::from_file(StatusCode::NotFound, "404.html")
    }
}
