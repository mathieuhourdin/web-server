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
    pub message_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
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
            message_id: None,
            user_id: None,
            display_name: None,
            reasoning_effort: None,
            verbosity: None,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = Some(display_name.into());
        self
    }

    pub fn with_message_id(mut self, message_id: Uuid) -> Self {
        self.message_id = Some(message_id);
        self
    }

    pub fn with_user_id(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
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
        match self.execute_with_user_prompt(&self.user_prompt).await {
            Ok(result) => Ok(result),
            Err(error) if is_incomplete_response_error(&error) => {
                tracing::warn!(
                    target: "work_analyzer",
                    display_name = self.display_name.as_deref().unwrap_or("unknown"),
                    analysis_id = ?self.analysis_id,
                    "gpt_incomplete_response_retrying_with_reduced_scope"
                );
                self.execute_with_user_prompt(&format!(
                    "{}\n\n{}",
                    self.user_prompt, REDUCED_SCOPE_RETRY_INSTRUCTION
                ))
                .await
            }
            Err(error) => Err(error),
        }
    }

    async fn execute_with_user_prompt<T>(&self, user_prompt: &str) -> Result<T, PpdcError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        Ok(make_gpt_request(
            self.model.clone(),
            self.reasoning_effort.clone(),
            self.verbosity.clone(),
            self.system_prompt.clone(),
            user_prompt.to_string(),
            self.schema.clone(),
            self.display_name.as_deref(),
            self.analysis_id,
            self.message_id,
            self.user_id,
        )
        .await?)
    }
}

const REDUCED_SCOPE_RETRY_INSTRUCTION: &str = "RETRY INSTRUCTION: The prior generation did not complete. Return a substantially smaller, concise, high-confidence result that still conforms exactly to the required schema. Prefer the most salient supported items; omit marginal, repetitive, or weakly supported extractions. Do not add explanation outside the required output.";

fn is_incomplete_response_error(error: &PpdcError) -> bool {
    error
        .message
        .starts_with("GPT response not completed, status=incomplete")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities_v2::error::ErrorType;

    #[test]
    fn recognizes_incomplete_responses_for_one_reduced_scope_retry() {
        let error = PpdcError::new(
            500,
            ErrorType::InternalError,
            "GPT response not completed, status=incomplete".to_string(),
        );
        assert!(is_incomplete_response_error(&error));
    }
}
