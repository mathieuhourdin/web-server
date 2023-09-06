use crate::entities::{user::User, error::PpdcError};
use crate::http::{HttpRequest, HttpResponse, StatusCode};

pub fn see_article(uuid: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let mut response = HttpResponse::from_file(StatusCode::Ok, "article_id.html")?;
    response.body = response.body.replace("INJECTED_ARTICLE_ID", format!("'{}'", uuid).as_str());
    Ok(response)
}

pub fn edit_article(uuid: &str, _request: &HttpRequest) -> Result<HttpResponse, PpdcError> {
    let mut response = HttpResponse::from_file(StatusCode::Ok, "edit-article-id.html")?;
    response.body = response.body.replace("INJECTED_ARTICLE_ID", format!("'{}'", uuid).as_str());
    Ok(response)
}

pub fn post_mathilde_route(request: &HttpRequest) -> Result<HttpResponse, PpdcError> {

    match &request.session.as_ref().unwrap().user_id {
        Some(user_id) => {
            let user_first_name = User::find(user_id)?.first_name;
            Ok(HttpResponse::created()
                .body(format!("Cool {user_first_name} but I already have one")))
        },
        None => Ok(HttpResponse::unauthorized()),
    }
    
}
