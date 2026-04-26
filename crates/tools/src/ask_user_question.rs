use async_trait::async_trait;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::tool::{
    AskUserQuestionOption, AskUserQuestionRequest, AskUserQuestionResponse, Tool, ToolContext,
    ToolError,
};

#[derive(Debug, Clone, Default)]
pub struct AskUserQuestionTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct AskUserQuestionInput {
    question: String,
    #[serde(default)]
    options: Vec<AskUserQuestionOption>,
    #[serde(default)]
    allow_custom: bool,
}

impl AskUserQuestionTool {
    pub fn new() -> Self {
        Self
    }

    fn validate_input(input: serde_json::Value) -> Result<AskUserQuestionRequest, ToolError> {
        let input: AskUserQuestionInput = serde_json::from_value(input)
            .map_err(|error| ToolError::InvalidInput(error.to_string()))?;

        if input.question.trim().is_empty() {
            return Err(ToolError::InvalidInput(
                "question cannot be empty".to_string(),
            ));
        }

        for option in &input.options {
            if option.label.trim().is_empty() {
                return Err(ToolError::InvalidInput(
                    "option label cannot be empty".to_string(),
                ));
            }
        }

        Ok(AskUserQuestionRequest {
            question: input.question,
            options: input.options,
            allow_custom: input.allow_custom,
        })
    }

    fn fallback_response(request: &AskUserQuestionRequest) -> Option<AskUserQuestionResponse> {
        request
            .options
            .first()
            .map(|option| AskUserQuestionResponse {
                selected_label: Some(option.label.clone()),
                answer: if option.description.trim().is_empty() {
                    option.label.clone()
                } else {
                    option.description.clone()
                },
                custom: false,
            })
    }

    fn validate_response(
        request: &AskUserQuestionRequest,
        response: AskUserQuestionResponse,
    ) -> Result<AskUserQuestionResponse, ToolError> {
        if response.answer.trim().is_empty() {
            return Err(ToolError::Execution(
                "question response cannot be empty".to_string(),
            ));
        }

        if response.custom {
            if !request.allow_custom {
                return Err(ToolError::Execution(
                    "custom answer was not allowed for this question".to_string(),
                ));
            }
            return Ok(AskUserQuestionResponse {
                selected_label: None,
                answer: response.answer,
                custom: true,
            });
        }

        let selected_label = response.selected_label.as_deref().ok_or_else(|| {
            ToolError::Execution("option response missing selected label".to_string())
        })?;
        if !request
            .options
            .iter()
            .any(|option| option.label == selected_label)
        {
            return Err(ToolError::Execution(format!(
                "unknown selected option label: {selected_label}"
            )));
        }

        Ok(response)
    }

    fn format_response(response: &AskUserQuestionResponse) -> String {
        serde_json::to_string(response).unwrap_or_else(|_| {
            format!(
                r#"{{"selected_label":null,"answer":{},"custom":{}}}"#,
                serde_json::to_string(&response.answer).unwrap_or_else(|_| "\"\"".to_string()),
                response.custom
            )
        })
    }
}

#[async_trait]
impl Tool for AskUserQuestionTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "AskUserQuestion".to_string(),
            description: "Ask the user a structured follow-up question with labeled options and optional custom input".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": { "type": "string" },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string" },
                                "description": { "type": "string" }
                            },
                            "required": ["label", "description"]
                        }
                    },
                    "allow_custom": { "type": "boolean" }
                },
                "required": ["question", "options", "allow_custom"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let request = Self::validate_input(input)?;

        let response = if let Some(callback) = context.user_question_callback {
            callback(request.clone()).await
        } else {
            Self::fallback_response(&request)
        };

        let Some(response) = response else {
            return Ok(ToolResult::error(
                context.tool_use_id,
                "Question cancelled or unavailable",
            ));
        };

        let response = Self::validate_response(&request, response)?;
        Ok(ToolResult::success(
            context.tool_use_id,
            Self::format_response(&response),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::UserQuestionCallback;
    use std::sync::Arc;

    fn base_input() -> serde_json::Value {
        serde_json::json!({
            "question": "Pick a mode",
            "options": [
                { "label": "Fast", "description": "Use the fast path" },
                { "label": "Careful", "description": "Use the careful path" }
            ],
            "allow_custom": true
        })
    }

    fn context_with_callback(callback: UserQuestionCallback) -> ToolContext {
        ToolContext {
            tool_use_id: "tool_1".to_string(),
            user_question_callback: Some(callback),
            ..ToolContext::default()
        }
    }

    #[test]
    fn schema_exposes_question_options_and_custom_flag() {
        let schema = AskUserQuestionTool::new().info().input_schema;
        assert!(schema["properties"].get("question").is_some());
        assert!(schema["properties"].get("options").is_some());
        assert!(schema["properties"].get("allow_custom").is_some());
    }

    #[tokio::test]
    async fn fallback_selects_first_option_without_callback() {
        let result = AskUserQuestionTool::new()
            .execute(
                base_input(),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    ..ToolContext::default()
                },
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let response: AskUserQuestionResponse = serde_json::from_str(&result.content).unwrap();
        assert_eq!(response.selected_label.as_deref(), Some("Fast"));
        assert_eq!(response.answer, "Use the fast path");
    }

    #[tokio::test]
    async fn callback_can_select_option() {
        let callback: UserQuestionCallback = Arc::new(|_| {
            Box::pin(async {
                Some(AskUserQuestionResponse {
                    selected_label: Some("Careful".to_string()),
                    answer: "Use the careful path".to_string(),
                    custom: false,
                })
            })
        });

        let result = AskUserQuestionTool::new()
            .execute(base_input(), context_with_callback(callback))
            .await
            .unwrap();

        let response: AskUserQuestionResponse = serde_json::from_str(&result.content).unwrap();
        assert_eq!(response.selected_label.as_deref(), Some("Careful"));
        assert!(!response.custom);
    }

    #[tokio::test]
    async fn callback_can_return_custom_answer() {
        let callback: UserQuestionCallback = Arc::new(|_| {
            Box::pin(async {
                Some(AskUserQuestionResponse {
                    selected_label: None,
                    answer: "Use my custom path".to_string(),
                    custom: true,
                })
            })
        });

        let result = AskUserQuestionTool::new()
            .execute(base_input(), context_with_callback(callback))
            .await
            .unwrap();

        let response: AskUserQuestionResponse = serde_json::from_str(&result.content).unwrap();
        assert_eq!(response.selected_label, None);
        assert_eq!(response.answer, "Use my custom path");
        assert!(response.custom);
    }

    #[tokio::test]
    async fn cancellation_returns_error_tool_result() {
        let callback: UserQuestionCallback = Arc::new(|_| Box::pin(async { None }));

        let result = AskUserQuestionTool::new()
            .execute(base_input(), context_with_callback(callback))
            .await
            .unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("cancelled"));
    }

    #[tokio::test]
    async fn missing_callback_without_options_returns_error_result() {
        let result = AskUserQuestionTool::new()
            .execute(
                serde_json::json!({
                    "question": "What now?",
                    "options": [],
                    "allow_custom": true
                }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    ..ToolContext::default()
                },
            )
            .await
            .unwrap();

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn invalid_input_rejects_empty_question() {
        let error = AskUserQuestionTool::new()
            .execute(
                serde_json::json!({
                    "question": "",
                    "options": [],
                    "allow_custom": false
                }),
                ToolContext::default(),
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ToolError::InvalidInput(_)));
    }
}
