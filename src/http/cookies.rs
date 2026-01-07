use super::server_url::ServerUrl;
use chrono::NaiveDateTime;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Cookie {
    pub data: HashMap<String, String>,
}

impl Cookie {
    pub fn from(string_cookie: Option<&String>) -> Cookie {
        match string_cookie {
            None => Cookie {
                data: HashMap::new(),
            },
            Some(value) => {
                let mut data_hashmap = HashMap::new();
                let splitted_values = value.split("; ").collect::<Vec<&str>>();
                for value in splitted_values {
                    let splitted_cookie_value = value.split("=").collect::<Vec<&str>>();
                    if let Some(key) = splitted_cookie_value.get(0) {
                        if let Some(value) = splitted_cookie_value.get(1) {
                            data_hashmap.insert(key.to_string(), value.to_string());
                        }
                    }
                }
                Cookie { data: data_hashmap }
            }
        }
    }
}

pub struct CookieValue {
    pub key: String,
    pub value: String,
    pub expires: NaiveDateTime,
    pub domain: String,
    pub path: String,
    pub same_site: String,
    pub secure: bool,
}

impl CookieValue {
    pub fn new(key: String, value: String, expires: NaiveDateTime) -> CookieValue {
        let domain = ServerUrl::from("http://localhost:5173".to_string()).host;
        let path = "/".to_string();
        let same_site = "Lax".to_string();
        let secure = true;
        CookieValue {
            key,
            value,
            expires,
            domain,
            path,
            same_site,
            secure,
        }
    }

    pub fn format(&self) -> String {
        let datetime: NaiveDateTime = self.expires.into();
        let expires = datetime.format("%a, %d-%b-%Y %H:%M:%S GMT");
        format!(
            "{}={}; Expires={expires}; Domain={}; Path={}; SameSite={}",
            self.key, self.value, self.domain, self.path, self.same_site
        )
    }
}
