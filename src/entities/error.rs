use std::fmt;
use diesel::result::Error as DieselError;
use serde_json::Error as SerdeError;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct PpdcError {
    pub status_code: u32,
    #[serde(skip_serializing)]
    pub error_type: ErrorType,
    pub message: String,
}

#[derive(Deserialize)]
pub enum ErrorType {
    InternalError,
    DatabaseError,
    ApiError,
}

impl PpdcError {
    pub fn new(status_code: u32, error_type: ErrorType, message: String) -> PpdcError {
        println!("status code: {status_code}; message : {message}");
        PpdcError { status_code, error_type, message }
    }
}

impl fmt::Display for PpdcError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl fmt::Debug for PpdcError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.message.as_str())
    }
}

impl From<DieselError> for PpdcError {
    fn from(error: DieselError) -> PpdcError {
        match error {
            DieselError::DatabaseError(_, err) => PpdcError::new(409, ErrorType::DatabaseError, err.message().to_string()),
            DieselError::NotFound => PpdcError::new(404, ErrorType::ApiError, "Record not found".to_string()),
            err => PpdcError::new(500, ErrorType::DatabaseError, format!("Diesel error: {}", err)),
        }
    }
}
impl From<SerdeError> for PpdcError {
    fn from(error: SerdeError) -> PpdcError {
        PpdcError::new(400, ErrorType::ApiError, format!("serde error : {}", error))
    }
}

/*impl From<std::error::Error> for PpdcError {
    fn from(error: std::error::Error) -> PpdcError {
        PpdcError::new(400, ErrorType::InternalError, error.to_string())
    }
}*/
