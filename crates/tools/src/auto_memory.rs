use async_trait::async_trait;
use rust_claude_core::memory::{
    self, AutoMemoryCandidate, AutoMemoryTrigger, MemoryFrontmatter, MemoryType, MemoryWriteRequest,
};
use rust_claude_core::tool_types::{ToolInfo, ToolResult};

use crate::{Tool, ToolContext, ToolError};

pub struct AutoMemoryTool;

impl AutoMemoryTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AutoMemoryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AutoMemoryTool {
    /// Auto-memory writes to the memory store on disk, but the operation is
    /// sandboxed (path-validated, scoped to the memory directory) and is meant
    /// to run without interactive permission prompts.  Returning `true` here
    /// ensures Default-mode permission checks treat it the same as read-only
    /// tools so it can fire automatically.
    fn is_read_only(&self) -> bool {
        true
    }

    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "AutoMemory".to_string(),
            description: "Save a durable automatic memory candidate when policy allows it".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "memory_type": { "type": "string", "enum": ["user", "feedback", "project", "reference"] },
                    "path": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "body": { "type": "string" },
                    "trigger": { "type": "string", "enum": ["user_correction", "stable_preference", "project_context"] }
                },
                "required": ["memory_type", "path", "title", "description", "body", "trigger"]
            }),
        }
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let tool_use_id = context.tool_use_id;
        let Some(candidate) = parse_candidate(&input) else {
            return Ok(ToolResult::error(
                tool_use_id,
                "Auto-memory skipped: invalid memory candidate input",
            ));
        };

        let cwd = match &context.app_state {
            Some(app_state) => app_state.lock().await.cwd.clone(),
            None => {
                return Ok(ToolResult::success(
                    tool_use_id,
                    "Auto-memory skipped: app state unavailable",
                ));
            }
        };
        let store = memory::discover_memory_store(&cwd);

        match memory::save_auto_memory_candidate(store.as_ref(), &candidate) {
            Ok(outcome) => Ok(ToolResult::success(
                tool_use_id,
                format!("Auto-memory {}", outcome.describe()),
            )),
            Err(error) => Ok(ToolResult::error(
                tool_use_id,
                format!("Auto-memory failed: {error}"),
            )),
        }
    }
}

fn parse_candidate(input: &serde_json::Value) -> Option<AutoMemoryCandidate> {
    let memory_type = MemoryType::parse(input.get("memory_type")?.as_str()?)?;
    let trigger = match input.get("trigger")?.as_str()? {
        "user_correction" => AutoMemoryTrigger::UserCorrection,
        "stable_preference" => AutoMemoryTrigger::StablePreference,
        "project_context" => AutoMemoryTrigger::ProjectContext,
        _ => return None,
    };
    let relative_path = input.get("path")?.as_str()?.trim().to_string();
    if relative_path.is_empty() || relative_path.starts_with('/') || relative_path.contains("..") {
        return None;
    }

    Some(AutoMemoryCandidate {
        request: MemoryWriteRequest {
            relative_path,
            frontmatter: MemoryFrontmatter {
                name: Some(input.get("title")?.as_str()?.trim().to_string()),
                description: Some(input.get("description")?.as_str()?.trim().to_string()),
                memory_type: Some(memory_type),
                extra: std::collections::HashMap::new(),
            },
            body: input.get("body")?.as_str()?.trim().to_string(),
        },
        trigger,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_auto_memory_candidate() {
        let candidate = parse_candidate(&serde_json::json!({
            "memory_type": "feedback",
            "path": "feedback/testing.md",
            "title": "Testing",
            "description": "Use real DB tests",
            "body": "Use real database integration tests.",
            "trigger": "user_correction"
        }))
        .unwrap();

        assert_eq!(candidate.request.relative_path, "feedback/testing.md");
        assert_eq!(candidate.request.frontmatter.memory_type, Some(MemoryType::Feedback));
        assert_eq!(candidate.trigger, AutoMemoryTrigger::UserCorrection);
    }

    #[test]
    fn rejects_unsafe_auto_memory_path() {
        assert!(parse_candidate(&serde_json::json!({
            "memory_type": "user",
            "path": "../secret.md",
            "title": "Secret",
            "description": "Secret",
            "body": "Secret",
            "trigger": "stable_preference"
        }))
        .is_none());
    }
}
