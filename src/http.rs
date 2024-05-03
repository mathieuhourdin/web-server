use std::collections::HashMap;
use std::fs;
use crate::entities::error::{PpdcError};
pub use cookies::{Cookie, CookieValue};
pub use request::{HttpRequest, ContentType};
pub use response::{HttpResponse, StatusCode};

pub mod cookies;
pub mod server_url;
pub mod request;
pub mod response;

