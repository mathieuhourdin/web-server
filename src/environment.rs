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

pub fn get_app_base_url() -> String {
    dotenv().ok();
    std::env::var("APP_BASE_URL").expect("APP_BASE_URL should be provided")
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

pub fn get_resend_api_key() -> String {
    dotenv().ok();
    std::env::var("RESEND_API_KEY")
        .or_else(|_| std::env::var("resend_api_key"))
        .expect("RESEND_API_KEY should be provided")
}

pub fn get_resend_api_base_url() -> String {
    dotenv().ok();
    std::env::var("RESEND_API_BASE_URL")
        .or_else(|_| std::env::var("resend_api_base_url"))
        .unwrap_or_else(|_| "https://api.resend.com".to_string())
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

pub fn get_observability_mode() -> String {
    dotenv().ok();
    std::env::var("OBSERVABILITY_MODE").unwrap_or_else(|_| "redacted".to_string())
}

pub fn get_gcs_bucket_name() -> String {
    dotenv().ok();
    std::env::var("GCS_BUCKET_NAME").expect("GCS_BUCKET_NAME should be provided")
}

pub fn get_assets_signed_url_ttl_seconds() -> u64 {
    dotenv().ok();
    std::env::var("ASSETS_SIGNED_URL_TTL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3600)
}

pub fn get_internal_cron_token() -> String {
    dotenv().ok();
    std::env::var("INTERNAL_CRON_TOKEN").expect("INTERNAL_CRON_TOKEN should be provided")
}

pub fn get_journal_share_link_hmac_secret() -> String {
    dotenv().ok();
    std::env::var("JOURNAL_SHARE_LINK_HMAC_SECRET")
        .or_else(|_| std::env::var("INTERNAL_CRON_TOKEN"))
        .expect("JOURNAL_SHARE_LINK_HMAC_SECRET or INTERNAL_CRON_TOKEN should be provided")
}
