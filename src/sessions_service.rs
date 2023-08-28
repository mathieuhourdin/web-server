use crate::http::{HttpRequest, HttpResponse, StatusCode};
use serde::{Serialize, Deserialize};
use serde_json;
use crate::entities::{user::User, session::Session, session::NewSession, error::PpdcError};
use crate::http::{CookieValue};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct LoginCheck {
    username: String,
    password: String,
}

pub async fn post_session_route(request: &mut HttpRequest) -> Result<HttpResponse, PpdcError> { 
    let login_check = serde_json::from_str::<LoginCheck>(&request.body[..])?;

    let existing_user = User::find_by_username(&login_check.username)?;

    let is_valid_password = existing_user.verify_password(&login_check.password.as_bytes())?;

    if is_valid_password {
        println!("Password is valid. Let's authenticate session");
        request
            .session
            .as_mut()
            .expect("request should have a session")
            .set_authenticated_and_user_id(existing_user.id);

        Session::update(request.session.as_ref().unwrap())?;

        let mut response = HttpResponse::new(StatusCode::Ok, serde_json::to_string(request.session.as_ref().unwrap()).unwrap());
        response.headers.insert("Set-Cookie".to_string(), format!("user_id={}", existing_user.id.clone()));
        return Ok(response);
        
    } else {
        return Ok(HttpResponse::new(StatusCode::BadRequest, String::from("Invalid password")));
    }
}

pub async fn get_session_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    Ok(HttpResponse::new(
        StatusCode::Ok,
        serde_json::to_string(request.session.as_ref().unwrap()).unwrap()
    ))
}

pub fn decode_session_id(session_id: &String) -> String {
    String::from(&session_id[..])
}

pub async fn create_or_attach_session(request: &mut HttpRequest) -> Result<String, PpdcError> {

    match request.cookies.data.get("session_id") {
        Some(value) => {
            let decoded_session_id = decode_session_id(value);
            let found_session = match Session::find(&Uuid::parse_str(&decoded_session_id[..]).unwrap()) {
                Err(err) => {
                    println!("Error when searching session for session_id: {decoded_session_id}, error : {:#?}", err);
                    let new_session = NewSession::new();
                    Session::create(&new_session)?
                },
                Ok(session) => {
                        if session.is_valid() {
                            session
                        } else {
                            println!("Session {} not valid anymore", session.id);
                            let new_session = NewSession::new();
                            Session::create(&new_session)?
                        }
                    }
            };
            let session_uuid = format!("{}", found_session.id);
            let response = CookieValue::new("session_id".to_string(), session_uuid, found_session.expires_at).format();
            request.session = Some(found_session);
            Ok(response)
        },
        None => {
            let new_session = NewSession::new();
            let new_session = Session::create(&new_session)?;
            let session_id = format!("{}", new_session.id);
            let response = CookieValue::new("session_id".to_string(), session_id, new_session.expires_at).format();
            request.session = Some(new_session);
            Ok(response)
        }
    }
}
