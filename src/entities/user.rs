use serde::{Serialize, Deserialize};
use argon2::{Config};
use rand::Rng;
use crate::entities::error::{PpdcError, ErrorType};

#[derive(Serialize, Deserialize, Clone)]
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
    pub fn hash_password(&mut self) -> Result<(), PpdcError> {
        let salt: [u8; 32] = rand::thread_rng().gen();
        let config = Config::default();

        self.password = argon2::hash_encoded(self.password.as_bytes(), &salt, &config)
            .map_err(|err| PpdcError::new(
                    500,
                    ErrorType::InternalError,
                    format!("Unable to encode password: {:#?}", err)))?;
        Ok(())
    }

    pub fn verify_password(&mut self, tested_password_bytes: &[u8]) -> Result<bool, PpdcError> {
        argon2::verify_encoded(&self.password, tested_password_bytes).map_err(|err| PpdcError::new(
                500,
                ErrorType::InternalError,
                format!("Unable to decode password: {:#?}", err)))
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

