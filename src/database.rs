use reqwest;
use reqwest::Error as ReqwestError;
use serde::{Serialize, Deserialize};
use crate::entities::{article::Article, user::User, session::Session};
use std::collections::HashMap;
use crate::entities::error::{PpdcError, ErrorType};
use crate::environment::get_couch_database_url;

#[derive(Serialize, Deserialize)]
pub struct Document {
    pub _id: String,
    pub _rev: String,
}

#[derive(Serialize, Deserialize)]
pub struct AllDocsResponse {
    pub total_rows: u32,
    pub offset: u32,
    pub rows: Vec<AllDocsResponseContent>,
}

#[derive(Serialize, Deserialize)]
pub struct AllDocsResponseContent {
    pub id: String,
    pub key: String,
    pub value: HashMap<String, String>,
    pub doc: Article,
}

#[derive(Serialize, Deserialize)]
pub struct UuidsResponse {
    pub uuids: Vec<String>
}

impl UuidsResponse {
    pub fn get_one(&self) -> String {
        self.uuids[0].clone()
    }
}

pub async fn health_check() -> Result<String, ReqwestError> {
    let response = reqwest::get(get_couch_database_url()).await?;
    let body = response.text().await?;
    println!("Body : {}", body);
    Ok(body)
}

pub async fn get_new_uuids() -> Result<UuidsResponse, ReqwestError> {
    let full_url = format!("{}{}", get_couch_database_url(), "/_uuids");
    reqwest::get(full_url).await?.json::<UuidsResponse>().await
}


pub async fn get_new_uuid() -> Result<String, ReqwestError> {
    let new_uuids = get_new_uuids().await;

    match new_uuids {
        Err(err) => Err(err),
        Ok(value) => Ok(value.get_one()),
    }
}

/*pub async fn create_user(user : &mut User) -> Result<String, ReqwestError> {
    let uuid = get_new_uuid().await?;
    user.uuid = Some(uuid.clone());
    let client = reqwest::Client::new();
    let response = client
        .put(format!("{}/users/{uuid}", get_couch_database_url()))
        .json::<User>(user)
        .send()
        .await?;
    let response_body = response.text().await?;
    Ok(response_body)
}*/

#[derive(Serialize, Deserialize)]
struct SelectorPayload {
    selector: HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
struct FindResult {
    docs: Option<Vec<User>>,
}

pub async fn find_user_by_username(username: &str) -> Result<Option<User>, PpdcError> {
    let full_url = format!("{}/users/_find", get_couch_database_url());
    let mut selection_hashmap = HashMap::new();
    selection_hashmap.insert(String::from("email"), String::from(username));
    let json_payload = SelectorPayload { selector: selection_hashmap };
    let client = reqwest::Client::new();
    let response = client
        .post(full_url)
        .json::<SelectorPayload>(&json_payload)
        .send()
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Error with db: {:#?}", err)))?;
    let response_body = response.json::<FindResult>().await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Could not decode response : {:#?}", err)))?;
    let user_array = response_body.docs.unwrap();
    if user_array.len() > 1 {
        return Err(PpdcError::new(500, ErrorType::InternalError, String::from("Should have only one user with this username")));
    }
    let first_user = user_array.first().cloned();
    Ok(first_user)
}

pub async fn get_user_by_uuid(uuid: &str) -> Result<User, PpdcError> {
    let full_url = format!("{}/users/{uuid}", get_couch_database_url());
    reqwest::get(full_url).await.unwrap().json::<User>().await.map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, err.to_string()))
}

pub async fn get_session_by_uuid(uuid: &str) -> Result<Option<Session>, PpdcError> {
    let full_url = format!("{}/sessions/{uuid}", get_couch_database_url());
    println!("Fill url : {full_url}");
    let db_response = reqwest::get(full_url)
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Error with db: {:#?}", err)))?;

    if db_response.status() == 404 {
        return Ok(None);
    }
    
    println!("{:#?}", db_response);
    let found_session = db_response.json::<Session>()
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Error while decoding session: {:#?}", err)))?;
    Ok(Some(found_session))
}

pub async fn get_revision_for_document(database: &str, object_id: String) -> Result<String, PpdcError> {
    let full_url = format!("{}/{database}/{object_id}", get_couch_database_url());
    let db_response = reqwest::get(full_url)
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Error with db: {:#?}", err)))?
        .json::<Document>()
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Error while decoding session: {:#?}", err)))?;
    Ok(db_response._rev)
}

pub async fn create_or_update_session(session: &Session) -> Result<(), PpdcError> {
    println!("create_or_update_session : new session persisting");
    let uuid = session.id;
    let client = reqwest::Client::new();
    let response = client.put(format!("{}/sessions/{uuid}", get_couch_database_url()))
        .json::<Session>(&session)
        .send()
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Error while persisting a new session: {:#?}", err)))?;
    if response.status().as_u16() >= 400 {
        return Err(PpdcError::new(500, ErrorType::DatabaseError, format!("Error while persisting a session: {}", response.text().await.unwrap())));
    }
    Ok(())
}

pub async fn update_session(session: &Session) -> Result<(), PpdcError> {
    println!("create_or_update_session : new session persisting");
    let uuid = session.id;
    let revision_id = get_revision_for_document("sessions", format!("{}", session.id)).await?;
    let client = reqwest::Client::new();
    let response = client.put(format!("{}/sessions/{uuid}", get_couch_database_url()))
        .json::<Session>(&session)
        .header("If-Match", revision_id)
        .send()
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Error while persisting a new session: {:#?}", err)))?;
    if response.status().as_u16() >= 400 {
        return Err(PpdcError::new(500, ErrorType::DatabaseError, format!("Error while persisting a session: {}", response.text().await.unwrap())));
    }
    Ok(())
}

pub async fn get_all_docs(database_name: &str, skip: u32, limit: u32) -> Result<AllDocsResponse, PpdcError> {
    let full_url = format!("{}/{database_name}/_all_docs?include_docs=true&skip={skip}&limit={limit}", get_couch_database_url());

    reqwest::get(full_url)
        .await
        .map_err(|err| PpdcError::new(
                500,
                ErrorType::DatabaseError,
                format!("Error whith get all for {database_name}, skip={skip} limit={limit}: {:#?}", err)))?
        .json::<AllDocsResponse>()
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Cant decode all_docs response: {:#?}", err)))
}

pub async fn get_articles(offset: u32, limit: u32) -> Result<Vec<Article>, PpdcError> {
    let all_docs_response = get_all_docs("articles", offset, limit).await?;
    Ok(all_docs_response.rows.iter().map(|value| value.doc.clone()).collect::<Vec<Article>>())
}

pub async fn get_article(article_uuid: &str) -> Result<Article, ReqwestError> {
    println!("Article uuid : {article_uuid}");
    let full_url = format!("{}/articles/{article_uuid}", get_couch_database_url());
    reqwest::get(full_url).await?.json::<Article>().await
}

pub async fn update_article(article: &Article) -> Result<(), PpdcError> {
    println!("create_or_update_session : new session persisting");
    let uuid = article.uuid.as_ref().unwrap();
    let revision_id = get_revision_for_document("articles", format!("{}", article.uuid.clone().unwrap())).await?;
    let client = reqwest::Client::new();
    let response = client.put(format!("{}/articles/{uuid}", get_couch_database_url()))
        .json::<Article>(&article)
        .header("If-Match", revision_id)
        .send()
        .await
        .map_err(|err| PpdcError::new(500, ErrorType::DatabaseError, format!("Error while persisting a new article: {:#?}", err)))?;
    if response.status().as_u16() >= 400 {
        return Err(PpdcError::new(500, ErrorType::DatabaseError, format!("Error while persisting a article: {}", response.text().await.unwrap())));
    }
    Ok(())
}

pub async fn create_article(article: &mut Article) -> Result<String, ReqwestError> {
    let uuid = get_new_uuid().await?;
    article.uuid = Some(uuid.clone());
    let payload = &serde_json::to_string(&article).unwrap();
    println!("Payload: {payload}");
    let client = reqwest::Client::new();
    let response = client.put(format!("{}/articles/{uuid}", get_couch_database_url()))
        .json::<Article>(article)
        .send()
        .await?;
    let response_body = response.text().await?;
    Ok(response_body)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_database_is_up() {
        health_check().await.expect("database should be up");
    }

    #[tokio::test]
    async fn test_get_new_uuid() {
        let new_uuid = get_new_uuid().await.expect("we should retrieve a uuid");
        assert_eq!(new_uuid.len(), 32);
    }
}
