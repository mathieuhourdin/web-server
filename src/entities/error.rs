use std::fmt;

pub struct PpdcError {
    status_code: u32,
    error_type: ErrorType,
    pub message: String,
}

pub enum ErrorType {
    InternalError,
    DatabaseError,
    ApiError,
}

impl PpdcError {
    pub fn new(status_code: u32, error_type: ErrorType, message: String) -> PpdcError {
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
