use std::collections::HashMap;

use rust_claude_core::tool_types::ToolInfo;

use crate::tool::{Tool, ToolContext, ToolError};

pub struct RegisteredTool {
    pub info: ToolInfo,
    pub is_read_only: bool,
    pub is_concurrency_safe: bool,
    pub tool: Box<dyn Tool>,
}

pub struct ToolRegistry {
    tools: HashMap<String, RegisteredTool>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            tools: HashMap::new(),
        }
    }

    pub fn register<T>(&mut self, tool: T)
    where
        T: Tool + 'static,
    {
        let info = tool.info();
        self.tools.insert(
            info.name.clone(),
            RegisteredTool {
                is_read_only: tool.is_read_only(),
                is_concurrency_safe: tool.is_concurrency_safe(),
                info,
                tool: Box::new(tool),
            },
        );
    }

    pub fn get(&self, name: &str) -> Option<&RegisteredTool> {
        self.tools.get(name)
    }

    pub fn list(&self) -> Vec<&RegisteredTool> {
        let mut tools: Vec<&RegisteredTool> = self.tools.values().collect();
        tools.sort_by(|a, b| a.info.name.cmp(&b.info.name));
        tools
    }

    pub async fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<rust_claude_core::tool_types::ToolResult, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::Execution(format!("unknown tool: {name}")))?;

        tool.tool.execute(input, context).await
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }

    pub fn is_concurrency_safe(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .map(|tool| tool.is_concurrency_safe)
            .unwrap_or(false)
    }

    /// Filter tools: keep only those in the allow list (if non-empty),
    /// then remove any in the deny list.
    pub fn apply_tool_filters(&mut self, allowed: &[String], disallowed: &[String]) {
        if !allowed.is_empty() {
            self.tools
                .retain(|name, _| allowed.iter().any(|a| a == name));
        }
        for name in disallowed {
            self.tools.remove(name);
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bash::BashTool;
    use crate::{FileEditTool, FileReadTool, FileWriteTool, GlobTool, GrepTool, TodoWriteTool};

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool::new());

        let tool = registry.get("Bash").unwrap();
        assert_eq!(tool.info.name, "Bash");
        assert!(!tool.is_concurrency_safe);
        assert!(registry.get("Unknown").is_none());
    }

    #[test]
    fn test_list_sorted() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool::new());
        registry.tools.insert(
            "FileWrite".to_string(),
            RegisteredTool {
                info: ToolInfo {
                    name: "FileWrite".to_string(),
                    description: "Write file".to_string(),
                    input_schema: serde_json::json!({}),
                },
                is_read_only: false,
                is_concurrency_safe: false,
                tool: Box::new(BashTool::new()),
            },
        );

        let names = registry.names();
        assert_eq!(names, vec!["Bash", "FileWrite"]);
    }

    #[tokio::test]
    async fn test_execute_registered_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool::new());

        let result = registry
            .execute(
                "Bash",
                serde_json::json!({ "command": "printf hello" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.content, "hello");
    }

    #[test]
    fn test_register_all_core_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(BashTool::new());
        registry.register(FileReadTool::new());
        registry.register(FileEditTool::new());
        registry.register(FileWriteTool::new());
        registry.register(GlobTool::new());
        registry.register(GrepTool::new());
        registry.register(TodoWriteTool::new());

        let names = registry.names();
        assert_eq!(
            names,
            vec![
                "Bash",
                "FileEdit",
                "FileRead",
                "FileWrite",
                "Glob",
                "Grep",
                "TodoWrite"
            ]
        );
    }
}
