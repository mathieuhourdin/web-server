use crate::entities::error::PpdcError;
pub use cookies::{Cookie, CookieValue};
pub use file::{File, FileType};
pub use request::{ContentType, HttpRequest};
pub use response::{HttpResponse, StatusCode};
use std::collections::HashMap;
use std::fs;

mod cookies;
mod file;
mod request;
mod response;
mod server_url;
