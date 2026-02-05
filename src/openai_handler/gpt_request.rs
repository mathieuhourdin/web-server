use crate::entities::error::PpdcError;
use crate::openai_handler::gpt_responses_handler::make_gpt_request;

pub struct GptRequestConfig {
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub schema: Option<serde_json::Value>,
    pub log_header: Option<String>,
}

impl GptRequestConfig {
    pub fn new(
        model: String,
        system_prompt: impl Into<String>,
        user_prompt: impl Into<String>,
        schema: Option<serde_json::Value>,
    ) -> Self {
        Self {
            model,
            system_prompt: system_prompt.into(),
            user_prompt: user_prompt.into(),
            schema,
            log_header: None,
        }
    }

    pub fn with_log_header(mut self, log_header: impl Into<String>) -> Self {
        self.log_header = Some(log_header.into());
        self
    }
    pub async fn execute<T>(&self) -> Result<T, PpdcError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        Ok(make_gpt_request(
            self.system_prompt.clone(), 
            self.user_prompt.clone(), 
            self.schema.clone(),
            self.log_header.as_deref(),
        ).await?)
    }
}
