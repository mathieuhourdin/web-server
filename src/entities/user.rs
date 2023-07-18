use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    pub uuid: Option<String>,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub hashed_password: String,
}
