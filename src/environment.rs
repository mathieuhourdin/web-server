extern crate dotenv;

use dotenv::dotenv;

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
