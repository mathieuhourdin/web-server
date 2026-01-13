use crate::entities::error::PpdcError;
use crate::openai_handler::gpt_handler::make_gpt_request;

pub struct GptRequestConfig {
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub schema: serde_json::Value,
}

impl GptRequestConfig {
    pub fn new(
        model: String,
        system_prompt: impl Into<String>,
        user_prompt: impl Into<String>,
        schema: serde_json::Value,
    ) -> Self {
        Self {
            model,
            system_prompt: system_prompt.into(),
            user_prompt: user_prompt.into(),
            schema,
        }
    }
    pub async fn execute<T>(&self) -> Result<T, PpdcError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        make_gpt_request(
            self.system_prompt.clone(), 
            self.user_prompt.clone(), 
            self.schema.clone()
        ).await?
    }
}