extern crate dotenv;

use dotenv::dotenv;

pub fn get_couch_database_url() -> String {
    dotenv().ok();
    std::env::var("COUCHDB_URL").expect("Database url should be set")
}

pub fn get_database_url() -> String {
    dotenv().ok();
    std::env::var("DATABASE_URL").expect("Database url should be set")
}

pub fn get_api_url() -> String {
    dotenv().ok();
    std::env::var("API_URL").expect("API url should be provided")
}

pub fn get_allow_origin() -> String {
    dotenv().ok();
    std::env::var("ALLOW_ORIGIN").expect("Allow origin shoud be provided")
}

pub fn get_openai_api_key() -> String {
    dotenv().ok();
    std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY should be provided")
}

pub fn get_openai_api_base_url() -> String {
    dotenv().ok();
    std::env::var("OPENAI_API_BASE_URL").unwrap_or_else(|_| "https://api.openai.com".to_string())
}

pub fn get_search_api_key() -> String {
    dotenv().ok();
    std::env::var("SEARCH_API_KEY").unwrap_or_else(|_| "".to_string())
}

pub fn get_search_engine_id() -> String {
    dotenv().ok();
    std::env::var("SEARCH_ENGINE_ID").unwrap_or_else(|_| "".to_string())
}

pub fn get_env() -> String {
    dotenv().ok();
    std::env::var("ENV").unwrap_or_else(|_| "development".to_string())
}
