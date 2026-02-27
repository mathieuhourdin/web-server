use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use diesel::result::Error as DieselError;
use serde::{Deserialize, Serialize};
use serde_json::Error as SerdeError;
use std::error::Error as StdError;
use std::fmt;

#[derive(PartialEq, Serialize, Deserialize)]
pub struct PpdcError {
    pub status_code: u32,
    #[serde(skip_serializing)]
    pub error_type: ErrorType,
    pub message: String,
}

#[derive(Debug, PartialEq, Deserialize)]
pub enum ErrorType {
    InternalError,
    DatabaseError,
    ApiError,
}

impl PpdcError {
    pub fn new(status_code: u32, error_type: ErrorType, message: String) -> PpdcError {
        println!("status code: {status_code}; message : {message}");
        PpdcError {
            status_code,
            error_type,
            message,
        }
    }

    pub fn unauthorized() -> PpdcError {
        Self::new(401, ErrorType::ApiError, "Unauthorized".into())
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

impl From<uuid::Error> for PpdcError {
    fn from(error: uuid::Error) -> PpdcError {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Incorrect uuid for ressource : {:#?}", error),
        )
    }
}

impl From<DieselError> for PpdcError {
    fn from(error: DieselError) -> PpdcError {
        match error {
            DieselError::DatabaseError(_, err) => {
                PpdcError::new(409, ErrorType::DatabaseError, err.message().to_string())
            }
            DieselError::NotFound => {
                PpdcError::new(404, ErrorType::ApiError, "Record not found".to_string())
            }
            err => PpdcError::new(
                500,
                ErrorType::DatabaseError,
                format!("Diesel error: {}", err),
            ),
        }
    }
}
impl From<SerdeError> for PpdcError {
    fn from(error: SerdeError) -> PpdcError {
        PpdcError::new(400, ErrorType::ApiError, format!("serde error : {}", error))
    }
}

impl From<Box<dyn StdError + Send + Sync>> for PpdcError {
    fn from(error: Box<dyn StdError + Send + Sync>) -> PpdcError {
        PpdcError::new(500, ErrorType::InternalError, error.to_string())
    }
}

impl IntoResponse for PpdcError {
    fn into_response(self) -> Response {
        let status = match self.error_type {
            ErrorType::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorType::ApiError => {
                StatusCode::from_u16(self.status_code as u16).unwrap_or(StatusCode::BAD_REQUEST)
            }
            ErrorType::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let error_json = serde_json::json!({
            "error": {
                "status_code": self.status_code,
                "message": self.message
            }
        });

        (status, Json(error_json)).into_response()
    }
}

/*impl From<std::error::Error> for PpdcError {
    fn from(error: std::error::Error) -> PpdcError {
        PpdcError::new(400, ErrorType::InternalError, error.to_string())
    }
}*/
