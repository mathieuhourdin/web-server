use crate::entities::error::PpdcError;
use crate::openai_handler::gpt_responses_handler::make_gpt_request;
use uuid::Uuid;

pub struct GptRequestConfig {
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub schema: Option<serde_json::Value>,
    pub analysis_id: Option<Uuid>,
    pub display_name: Option<String>,
}

impl GptRequestConfig {
    pub fn new(
        model: String,
        system_prompt: impl Into<String>,
        user_prompt: impl Into<String>,
        schema: Option<serde_json::Value>,
        analysis_id: Option<Uuid>,
    ) -> Self {
        Self {
            model,
            system_prompt: system_prompt.into(),
            user_prompt: user_prompt.into(),
            schema,
            analysis_id,
            display_name: None,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
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
            self.display_name.as_deref(),
            self.analysis_id,
        )
        .await?)
    }
}
