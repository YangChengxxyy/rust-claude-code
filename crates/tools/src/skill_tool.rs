//! `SkillTool` — load a local skill by name and return its prompt (iteration 51).
//!
//! The model calls `Skill(skill, args)` to pull a discovered skill's body into
//! its context. Discovery itself happens elsewhere
//! ([`rust_claude_core::skills::SkillRegistry`] / the SDK `SkillLoader`); this
//! tool just looks the skill up in the registry it was constructed with,
//! substitutes `{args}` into the body, and returns the rendered prompt.
//!
//! Missing skills yield a clear error. The tool performs no I/O and no state
//! mutation, so it is read-only and concurrency-safe.

use std::sync::Arc;

use async_trait::async_trait;
use rust_claude_core::skills::SkillRegistry;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use serde::Deserialize;

use crate::tool::{Tool, ToolContext, ToolError};

/// Load a local skill by name and return its rendered prompt.
#[derive(Debug, Clone)]
pub struct SkillTool {
    registry: Arc<SkillRegistry>,
}

#[derive(Debug, Clone, Deserialize)]
struct SkillInput {
    skill: String,
    #[serde(default)]
    args: Option<String>,
}

impl SkillTool {
    pub fn new(registry: Arc<SkillRegistry>) -> Self {
        Self { registry }
    }

    fn run(
        registry: &SkillRegistry,
        tool_use_id: &str,
        input: SkillInput,
    ) -> Result<ToolResult, ToolError> {
        let skill = registry.get(&input.skill).ok_or_else(|| {
            ToolError::Execution(format!("skill not found: {}", input.skill))
        })?;
        let args = input.args.unwrap_or_default();
        let body = substitute_args(&skill.body, &args);
        Ok(ToolResult::success(
            tool_use_id.to_string(),
            format!("Loaded skill '{}'\n\n{}", skill.name, body),
        ))
    }
}

/// Replace every `{args}` placeholder in `body` with `args`.
fn substitute_args(body: &str, args: &str) -> String {
    body.replace("{args}", args)
}

#[async_trait]
impl Tool for SkillTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Skill".to_string(),
            description: "Load a local skill by name and return its prompt so you can \
                follow the skill's instructions. Use `{args}` inside a skill body for an \
                optional arguments substitution. Returns an error if the skill is not found."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "skill": {
                        "type": "string",
                        "description": "Name of the local skill to load."
                    },
                    "args": {
                        "type": "string",
                        "description": "Optional arguments substituted into the skill body's `{args}` placeholders."
                    }
                },
                "required": ["skill"]
            }),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<ToolResult, ToolError> {
        let input: SkillInput =
            serde_json::from_value(input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        Self::run(&self.registry, &context.tool_use_id, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "skill-tool-test-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn load_registry(dir: &Path) -> SkillRegistry {
        SkillRegistry::load_from_dirs(&[(dir.to_path_buf(), rust_claude_core::skills::SkillSource::User)])
    }

    fn write_skill(dir: &Path, name: &str, body: &str) {
        let content = format!("---\nname: {name}\ndescription: d\n---\n{body}");
        std::fs::write(dir.join(format!("{name}.md")), content).unwrap();
    }

    #[test]
    fn finds_skill_and_renders_body() {
        let dir = temp_dir("find");
        write_skill(&dir, "deploy", "Run deploy steps.\n");
        let registry = load_registry(&dir);
        let r = SkillTool::run(
            &registry,
            "t",
            SkillInput {
                skill: "deploy".into(),
                args: None,
            },
        )
        .unwrap();
        assert!(r.content.contains("Loaded skill 'deploy'"));
        assert!(r.content.contains("Run deploy steps."));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn substitutes_args_placeholder() {
        let dir = temp_dir("args");
        write_skill(&dir, "greet", "Hello {args}!\n");
        let registry = load_registry(&dir);
        let r = SkillTool::run(
            &registry,
            "t",
            SkillInput {
                skill: "greet".into(),
                args: Some("world".into()),
            },
        )
        .unwrap();
        assert!(r.content.contains("Hello world!"));
        assert!(!r.content.contains("{args}"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_args_placeholder_renders_empty() {
        let dir = temp_dir("noargs");
        write_skill(&dir, "greet", "Hi {args}!\n");
        let registry = load_registry(&dir);
        let r = SkillTool::run(
            &registry,
            "t",
            SkillInput {
                skill: "greet".into(),
                args: None,
            },
        )
        .unwrap();
        // {args} with no args supplied -> empty string.
        assert!(r.content.contains("Hi !"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_skill_returns_clear_error() {
        let dir = temp_dir("missing");
        let registry = load_registry(&dir);
        let err = SkillTool::run(
            &registry,
            "t",
            SkillInput {
                skill: "ghost".into(),
                args: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, ToolError::Execution(_)));
        assert!(err.to_string().contains("ghost"));
    }

    #[test]
    fn tool_metadata_and_flags() {
        let dir = temp_dir("meta");
        let registry = load_registry(&dir);
        let tool = SkillTool::new(Arc::new(registry));
        assert_eq!(tool.info().name, "Skill");
        assert!(tool.is_read_only());
        assert!(tool.is_concurrency_safe());
    }

    #[tokio::test]
    async fn execute_uses_owned_registry() {
        let dir = temp_dir("exec");
        write_skill(&dir, "deploy", "Deploy it.\n");
        let tool = SkillTool::new(Arc::new(load_registry(&dir)));
        let result = tool
            .execute(
                serde_json::json!({ "skill": "deploy", "args": "prod" }),
                ToolContext {
                    tool_use_id: "t".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert!(result.content.contains("Deploy it."));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn execute_rejects_invalid_input() {
        let dir = temp_dir("badinput");
        let tool = SkillTool::new(Arc::new(load_registry(&dir)));
        // `skill` is required; an integer is the wrong type.
        let err = tool
            .execute(
                serde_json::json!({ "skill": 123 }),
                ToolContext {
                    tool_use_id: "t".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
