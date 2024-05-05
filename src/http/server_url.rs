
pub struct ServerUrl {
    pub protocol: String,
    pub host: String,
    pub port: Option<String>,
}

impl ServerUrl {
    pub fn from(url_string: String) -> ServerUrl {
        let splitted_first = url_string.split("://").collect::<Vec<&str>>();
        let protocol = splitted_first.get(0).expect("Should have a protocol value in url").to_string();
        let authority = splitted_first.get(1).expect("Should have an authority");
        let host = authority.split(":").collect::<Vec<_>>().get(0).expect("Should have a host").to_string();
        let port = authority.split(":").collect::<Vec<_>>().get(1).map(|value| value.to_string());
        ServerUrl { protocol, host, port }
    }

    #[allow(dead_code)]
    pub fn to_string_url(&self) -> String {
        match &self.port {
            Some(value) => format!("{}://{}:{}", self.protocol, self.host, value),
            None => format!("{}://{}", self.protocol, self.host),
        }
    }
}
