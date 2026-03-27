use crate::entities_v2::error::PpdcError;
use crate::openai_handler::gpt_responses_handler::{
    make_gpt_request, GptReasoningEffort, GptVerbosity,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct GptRequestConfig {
    pub model: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub schema: Option<serde_json::Value>,
    pub analysis_id: Option<Uuid>,
    pub display_name: Option<String>,
    pub reasoning_effort: Option<GptReasoningEffort>,
    pub verbosity: Option<GptVerbosity>,
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
            reasoning_effort: None,
            verbosity: None,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn with_reasoning_effort(mut self, reasoning_effort: GptReasoningEffort) -> Self {
        self.reasoning_effort = Some(reasoning_effort);
        self
    }

    pub fn with_verbosity(mut self, verbosity: GptVerbosity) -> Self {
        self.verbosity = Some(verbosity);
        self
    }

    pub async fn execute<T>(&self) -> Result<T, PpdcError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        Ok(make_gpt_request(
            self.model.clone(),
            self.reasoning_effort.clone(),
            self.verbosity.clone(),
            self.system_prompt.clone(),
            self.user_prompt.clone(),
            self.schema.clone(),
            self.display_name.as_deref(),
            self.analysis_id,
        )
        .await?)
    }
}
