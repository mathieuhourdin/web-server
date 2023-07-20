use serde::{Serialize, Deserialize};
use argon2::Config;
use rand::Rng;

#[derive(Serialize, Deserialize)]
pub struct User {
    pub uuid: Option<String>,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub handle: String,
    #[serde(skip_serializing)]
    pub password: String,
}

impl User {
    pub fn hash_password(&mut self) {
        let salt: [u8; 32] = rand::thread_rng().gen();
        let config = Config::default();

        self.password = argon2::hash_encoded(self.password.as_bytes(), &salt, &config).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_password() {
        let mut user = User {
            uuid: Some(String::from("uuid")),
            email: String::from("email"),
            first_name: String::from("first_name"),
            last_name: String::from("last_name"),
            handle: String::from("@handle"),
            password: String::from("password"),
        };
        user.hash_password();
        assert_ne!(user.password, String::from("password"));
    }
}

