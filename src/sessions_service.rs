use crate::http::{HttpRequest, HttpResponse, StatusCode};
use serde::{Serialize, Deserialize};
use serde_json;
use crate::entities::{user::User, session::Session, session::NewSession, error::PpdcError};
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

fn generate_valid_session_from_id(id: &str, request: &mut HttpRequest) -> Result<String, PpdcError> {

    println!("generate_valid_session_from_id id : {id}");
    let decoded_session_id = HttpRequest::parse_uuid(id)?;
    let found_session = match Session::find(&decoded_session_id) {
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
    println!("generate_valid_session_from_id found_session id {:#?}", found_session.id);
    let session_uuid = format!("{}", found_session.id);
    let response = CookieValue::new("session_id".to_string(), session_uuid, found_session.expires_at).format();
    request.session = Some(found_session);
    Ok(response)
}

pub async fn create_or_attach_session(request: &mut HttpRequest) -> Result<String, PpdcError> {

    if request.cookies.data.contains_key("session_id") {
        let session_id = request.cookies.data["session_id"].clone();
        generate_valid_session_from_id(&session_id[..], request)
    } else {
        if request.headers.contains_key("authorization") {
            let session_id = request.headers.get("authorization").unwrap().clone();
            let coming_response = generate_valid_session_from_id(&session_id[..], request);
            println!("create_or_attach_session finished generating");
            coming_response
        } else {
            let new_session = NewSession::new();
            let new_session = Session::create(&new_session)?;
            let session_id = format!("{}", new_session.id);
            let response = CookieValue::new("session_id".to_string(), session_id, new_session.expires_at).format();
            request.session = Some(new_session);
            Ok(response)
        }
    }
}
