use reqwest;
use reqwest::Error as ReqwestError;
use serde::{Serialize, Deserialize};
use crate::entities::article::Article;

const BASE_URL: &str = "http://admin:password@172.17.0.1:5984";

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
    let response = reqwest::get("http://172.17.0.1:5984").await?;
    let body = response.text().await?;
    println!("Body : {}", body);
    Ok(body)
}

pub async fn get_new_uuids() -> Result<UuidsResponse, ReqwestError> {
    let full_url = format!("{BASE_URL}{}", "/_uuids");
    reqwest::get(full_url).await?.json::<UuidsResponse>().await
}


pub async fn get_new_uuid() -> Result<String, ReqwestError> {
    let new_uuids = get_new_uuids().await;

    match new_uuids {
        Err(err) => Err(err),
        Ok(value) => Ok(value.get_one()),
    }
}

pub async fn get_article(article_uuid: &str) -> Result<Article, ReqwestError> {
    println!("Article uuid : {article_uuid}");
    let full_url = format!("{BASE_URL}/articles/{article_uuid}");
    reqwest::get(full_url).await?.json::<Article>().await
}

pub async fn create_article(article: &mut Article) -> Result<String, ReqwestError> {
    let uuid = get_new_uuid().await?;
    article.uuid = Some(uuid.clone());
    let payload = &serde_json::to_string(&article).unwrap();
    println!("Payload: {payload}");
    let client = reqwest::Client::new();
    let response = client.put(format!("{BASE_URL}/articles/{uuid}"))
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
