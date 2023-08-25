use crate::http::{HttpRequest, HttpResponse, StatusCode};
use serde::{Serialize, Deserialize};
use serde_json;
use crate::database;
use crate::entities::{user::User, session::Session, error::PpdcError};
use crate::http::{CookieValue};

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
            .set_user_id(existing_user.id);

        database::update_session(request.session.as_ref().unwrap()).await?;

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
            let found_session = match database::get_session_by_uuid(&decoded_session_id[..]).await {
                Err(err) => {
                    println!("Error when searching session for session_id: {decoded_session_id}, error : {:#?}", err);
                    let new_session = Session::new();
                    database::create_or_update_session(&new_session).await?;
                    new_session
                },
                Ok(value) => {
                    match value {
                        Some(session) => {
                            if session.is_valid() {
                                session
                            } else {
                                println!("Session {} not valid anymore", session.id);
                                let new_session = Session::new();
                                database::create_or_update_session(&new_session).await?;
                                new_session
                            }
                        },
                        None => {
                            println!("Cant find session for session_id: {decoded_session_id}");
                            let new_session = Session::new();
                            database::create_or_update_session(&new_session).await?;
                            new_session
                        },
                    }
                },
            };
            let session_uuid = format!("{}", found_session.id);
            let response = CookieValue::new("session_id".to_string(), session_uuid, found_session.expires_at).format();
            request.session = Some(found_session);
            Ok(response)
        },
        None => {
            let new_session = Session::new();
            database::create_or_update_session(&new_session).await?;
            let session_uuid = format!("{}", new_session.id);
            let response = CookieValue::new("session_id".to_string(), session_uuid, new_session.expires_at).format();
            request.session = Some(new_session);
            Ok(response)
        }
    }
}
