use serde::{Serialize, Deserialize};
use crate::entities::{user::User, session::Session, error::{ErrorType, PpdcError}};
use axum::{
    debug_handler,
    extract::{Json, Extension},
    body::Body,
    middleware::Next,
    http::{Request, StatusCode as AxumStatusCode},
    response::IntoResponse
};
use crate::db::DbPool;

#[derive(Serialize, Deserialize)]
pub struct LoginCheck {
    username: String,
    password: String,
}

pub async fn add_session_to_request(
    session: Session,
    mut req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    req.extensions_mut().insert(session.clone());
    next.run(req).await
}

pub async fn auth_middleware_custom(
    Extension(session): Extension<Session>,
    req: Request<Body>,
    next: Next,
) -> impl IntoResponse {
    if session.user_id.is_none() {
        return (AxumStatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    next.run(req).await
}


#[debug_handler]
pub async fn post_session_route(
    Extension(pool): Extension<DbPool>,
    Extension(mut session): Extension<Session>,
    Json(payload): Json<LoginCheck>,
) -> Result<Json<Session>, PpdcError> { 
    println!("Post session route");

    let existing_user = User::find_by_username(&payload.username, &pool)?;

    let is_valid_password = existing_user.verify_password(&payload.password.as_bytes())?;

    if is_valid_password {
        println!("Password is valid. Let's authenticate session");
        session.set_authenticated_and_user_id(existing_user.id);

        let session = Session::update(&session, &pool)?;

        return Ok(Json(session));
        
    } else {
        return Err(PpdcError::new(401, ErrorType::ApiError, String::from("Invalid password")));
    }
}

#[debug_handler]
pub async fn get_session_route(
    Extension(session): Extension<Session>
) -> Result<Json<Session>, PpdcError> {
    Ok(Json(session))
}

pub fn decode_session_id(session_id: &String) -> String {
    String::from(&session_id[..])
}
