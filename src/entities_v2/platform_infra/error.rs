use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use diesel::r2d2::PoolError;
use diesel::result::Error as DieselError;
use serde::{Deserialize, Serialize};
use serde_json::Error as SerdeError;
use std::error::Error as StdError;
use std::fmt;
use std::panic::Location;
use uuid::Uuid;

#[derive(PartialEq)]
pub struct PpdcError {
    pub status_code: u32,
    pub error_type: ErrorType,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub error_id: Uuid,
    origin_file: &'static str,
    origin_line: u32,
    origin_column: u32,
    operation: Option<String>,
    log_context: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorType {
    InternalError,
    DatabaseError,
    ApiError,
}

#[derive(Debug, Clone)]
pub(crate) struct PpdcErrorLogContext {
    pub error_id: Uuid,
    pub declared_status_code: u32,
    pub response_status_code: u16,
    pub error_type: &'static str,
    pub message: String,
    pub origin_file: &'static str,
    pub origin_line: u32,
    pub origin_column: u32,
    pub operation: Option<String>,
    pub context: serde_json::Value,
}

pub(crate) struct PpdcRequestLogContext<'a> {
    pub request_id: &'a str,
    pub method: &'a str,
    pub route: &'a str,
    pub user_id: &'a str,
    pub session_id: &'a str,
    pub latency_ms: u128,
}

impl PpdcErrorLogContext {
    pub(crate) fn emit(&self, request: Option<&PpdcRequestLogContext<'_>>, event: &str) {
        let operation = self.operation.as_deref().unwrap_or("");
        let context = self.context.to_string();
        let request_id = request.map(|request| request.request_id).unwrap_or("");
        let http_method = request.map(|request| request.method).unwrap_or("");
        let http_route = request.map(|request| request.route).unwrap_or("");
        let user_id = request.map(|request| request.user_id).unwrap_or("");
        let session_id = request.map(|request| request.session_id).unwrap_or("");
        let latency_ms = request
            .map(|request| request.latency_ms)
            .unwrap_or_default();
        if self.response_status_code >= 500 {
            tracing::error!(
                target: "api",
                request_id,
                http_method,
                http_route,
                user_id,
                session_id,
                error_id = %self.error_id,
                status_code = self.response_status_code,
                declared_status_code = self.declared_status_code,
                error_type = self.error_type,
                operation,
                error_origin_file = self.origin_file,
                error_origin_line = self.origin_line,
                error_origin_column = self.origin_column,
                error_context = %context,
                latency_ms,
                error_message = %self.message,
                "{}",
                event
            );
        } else {
            tracing::warn!(
                target: "api",
                request_id,
                http_method,
                http_route,
                user_id,
                session_id,
                error_id = %self.error_id,
                status_code = self.response_status_code,
                declared_status_code = self.declared_status_code,
                error_type = self.error_type,
                operation,
                error_origin_file = self.origin_file,
                error_origin_line = self.origin_line,
                error_origin_column = self.origin_column,
                error_context = %context,
                latency_ms,
                error_message = %self.message,
                "{}",
                event
            );
        }
    }
}

impl PpdcError {
    #[track_caller]
    pub fn new(status_code: u32, error_type: ErrorType, message: String) -> PpdcError {
        let caller = Location::caller();
        PpdcError {
            status_code,
            error_type,
            message,
            details: None,
            error_id: Uuid::new_v4(),
            origin_file: caller.file(),
            origin_line: caller.line(),
            origin_column: caller.column(),
            operation: None,
            log_context: serde_json::Map::new(),
        }
    }

    #[track_caller]
    pub fn unauthorized() -> PpdcError {
        Self::new(401, ErrorType::ApiError, "Unauthorized".into())
    }

    pub fn with_details(mut self, details: serde_json::Value) -> PpdcError {
        self.details = Some(details);
        self
    }

    pub fn with_context(mut self, operation: impl Into<String>) -> PpdcError {
        self.operation = Some(operation.into());
        self
    }

    pub fn with_log_field(mut self, key: impl Into<String>, value: impl Serialize) -> PpdcError {
        let value = serde_json::to_value(value).unwrap_or_else(|error| {
            serde_json::Value::String(format!("<failed to serialize log field: {}>", error))
        });
        self.log_context.insert(key.into(), value);
        self
    }

    pub fn log(&self, event: &str) {
        self.log_context().emit(None, event);
    }

    fn response_status(&self) -> StatusCode {
        match self.error_type {
            ErrorType::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorType::ApiError => {
                StatusCode::from_u16(self.status_code as u16).unwrap_or(StatusCode::BAD_REQUEST)
            }
            ErrorType::DatabaseError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn log_context(&self) -> PpdcErrorLogContext {
        PpdcErrorLogContext {
            error_id: self.error_id,
            declared_status_code: self.status_code,
            response_status_code: self.response_status().as_u16(),
            error_type: match self.error_type {
                ErrorType::InternalError => "internal",
                ErrorType::DatabaseError => "database",
                ErrorType::ApiError => "api",
            },
            message: self.message.clone(),
            origin_file: self.origin_file,
            origin_line: self.origin_line,
            origin_column: self.origin_column,
            operation: self.operation.clone(),
            context: serde_json::Value::Object(self.log_context.clone()),
        }
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
    #[track_caller]
    fn from(error: uuid::Error) -> PpdcError {
        PpdcError::new(
            400,
            ErrorType::ApiError,
            format!("Incorrect uuid for ressource : {:#?}", error),
        )
    }
}

impl From<DieselError> for PpdcError {
    #[track_caller]
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

impl From<PoolError> for PpdcError {
    #[track_caller]
    fn from(error: PoolError) -> PpdcError {
        PpdcError::new(
            500,
            ErrorType::DatabaseError,
            format!("Database pool error: {}", error),
        )
    }
}

impl From<SerdeError> for PpdcError {
    #[track_caller]
    fn from(error: SerdeError) -> PpdcError {
        PpdcError::new(400, ErrorType::ApiError, format!("serde error : {}", error))
    }
}

impl From<Box<dyn StdError + Send + Sync>> for PpdcError {
    #[track_caller]
    fn from(error: Box<dyn StdError + Send + Sync>) -> PpdcError {
        PpdcError::new(500, ErrorType::InternalError, error.to_string())
    }
}

impl IntoResponse for PpdcError {
    fn into_response(self) -> Response {
        let status = self.response_status();
        let log_context = self.log_context();

        let mut error_object = serde_json::json!({
            "status_code": self.status_code,
            "message": self.message
        });
        if status.is_server_error() {
            error_object["error_id"] = serde_json::json!(self.error_id);
        }
        if let Some(details) = self.details {
            error_object["details"] = details;
        }
        let error_json = serde_json::json!({
            "error": error_object
        });

        let mut response = (status, Json(error_json)).into_response();
        response.extensions_mut().insert(log_context);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_the_error_creation_site() {
        let expected_line = line!() + 1;
        let error = PpdcError::new(500, ErrorType::InternalError, "failure".to_string());

        assert_eq!(error.origin_file, file!());
        assert_eq!(error.origin_line, expected_line);
    }

    #[test]
    fn captures_the_conversion_site_for_wrapped_errors() {
        let expected_line = line!() + 1;
        let error: PpdcError = DieselError::NotFound.into();

        assert_eq!(error.origin_file, file!());
        assert_eq!(error.origin_line, expected_line);
    }

    #[test]
    fn preserves_structured_operational_context() {
        let resource_id = Uuid::new_v4();
        let error = PpdcError::new(500, ErrorType::InternalError, "failure".to_string())
            .with_context("load_resource")
            .with_log_field("resource_id", resource_id);
        let context = error.log_context();

        assert_eq!(context.operation.as_deref(), Some("load_resource"));
        assert_eq!(context.context["resource_id"], resource_id.to_string());
    }

    #[tokio::test]
    async fn server_error_response_exposes_error_id_and_keeps_log_metadata_private() {
        let error = PpdcError::new(500, ErrorType::InternalError, "failure".to_string());
        let error_id = error.error_id;
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .extensions()
                .get::<PpdcErrorLogContext>()
                .map(|context| context.error_id),
            Some(error_id)
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"]["error_id"], error_id.to_string());
        assert!(body["error"].get("origin_file").is_none());
        assert!(body["error"].get("context").is_none());
    }

    #[tokio::test]
    async fn client_error_response_does_not_expose_error_id() {
        let response =
            PpdcError::new(404, ErrorType::ApiError, "missing".to_string()).into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert!(body["error"].get("error_id").is_none());
    }
}

/*impl From<std::error::Error> for PpdcError {
    fn from(error: std::error::Error) -> PpdcError {
        PpdcError::new(400, ErrorType::InternalError, error.to_string())
    }
}*/
