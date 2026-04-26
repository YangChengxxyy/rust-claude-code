use serde::{Deserialize, Serialize};

use crate::message::Usage;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub model: String,
    pub model_setting: String,
    pub cwd: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub first_user_summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSnapshot {
    pub model: String,
    pub context_capacity: Option<u32>,
    pub used_tokens: u32,
    pub system_prompt_tokens: u32,
    pub message_tokens: u32,
    pub tool_result_tokens: u32,
    pub remaining_tokens: Option<u32>,
}
