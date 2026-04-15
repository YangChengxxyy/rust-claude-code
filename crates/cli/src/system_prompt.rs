//! Compose a Claude Code-style system prompt.
//!
//! The prompt is built from:
//! - Core behavior guidelines and tool usage instructions
//! - Available tool descriptions
//! - Current working directory, OS info, and date context

use rust_claude_tools::ToolRegistry;
use std::path::Path;

/// Build a complete system prompt suitable for Claude Code-style operation.
pub fn build_system_prompt(
    cwd: &Path,
    tools: &ToolRegistry,
    custom_append: Option<&str>,
) -> String {
    let mut parts = Vec::new();

    // Core identity and behavior
    parts.push(CORE_PROMPT.to_string());

    // Tool descriptions
    let tool_section = build_tool_section(tools);
    if !tool_section.is_empty() {
        parts.push(tool_section);
    }

    // Environment context
    parts.push(build_environment_section(cwd));

    // Custom append (from --append-system-prompt)
    if let Some(append) = custom_append {
        parts.push(append.to_string());
    }

    parts.join("\n\n")
}

const CORE_PROMPT: &str = r#"You are an AI assistant helping with software engineering tasks. You have access to tools for reading files, editing files, writing files, running bash commands, and managing todo lists.

# Guidelines

- When asked to modify code, read the relevant file first to understand the context.
- Use the appropriate tool for each task (Bash for commands, FileRead for reading, FileEdit for editing, FileWrite for creating new files).
- Be concise in your responses. Explain what you did and why.
- When running bash commands, prefer non-destructive operations. Ask for confirmation before dangerous operations like `rm -rf`.
- For file edits, use FileEdit with precise old_string/new_string replacements. Ensure old_string is unique in the file.
- Always verify your changes by reading the modified file or running relevant tests.
- Do not make changes beyond what was asked. A bug fix doesn't need surrounding code cleaned up.
- Prioritize writing safe, secure, and correct code."#;

fn build_tool_section(tools: &ToolRegistry) -> String {
    let tool_list = tools.list();
    if tool_list.is_empty() {
        return String::new();
    }

    let mut lines = vec!["# Available Tools".to_string(), String::new()];

    for tool in tool_list {
        lines.push(format!(
            "- **{}**: {}{}",
            tool.info.name,
            tool.info.description,
            if tool.is_read_only {
                " (read-only)"
            } else {
                ""
            }
        ));
    }

    lines.join("\n")
}

fn build_environment_section(cwd: &Path) -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();

    format!(
        "# Environment\n\n- Working directory: {}\n- OS: {} ({})\n- Date: {}",
        cwd.display(),
        os,
        arch,
        date,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_tools::{BashTool, FileReadTool};

    #[test]
    fn test_build_system_prompt_contains_core() {
        let cwd = std::path::PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(&cwd, &tools, None);
        assert!(prompt.contains("AI assistant"));
        assert!(prompt.contains("software engineering"));
    }

    #[test]
    fn test_build_system_prompt_contains_environment() {
        let cwd = std::path::PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(&cwd, &tools, None);
        assert!(prompt.contains("/tmp/test"));
        assert!(prompt.contains("Date:"));
    }

    #[test]
    fn test_build_system_prompt_contains_tools() {
        let cwd = std::path::PathBuf::from("/tmp/test");
        let mut tools = ToolRegistry::new();
        tools.register(BashTool::new());
        tools.register(FileReadTool::new());
        let prompt = build_system_prompt(&cwd, &tools, None);
        assert!(prompt.contains("Bash"));
        assert!(prompt.contains("FileRead"));
        assert!(prompt.contains("Available Tools"));
    }

    #[test]
    fn test_build_system_prompt_with_custom_append() {
        let cwd = std::path::PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(&cwd, &tools, Some("Custom instructions here"));
        assert!(prompt.contains("Custom instructions here"));
    }

    #[test]
    fn test_build_tool_section_empty_registry() {
        let tools = ToolRegistry::new();
        let section = build_tool_section(&tools);
        assert!(section.is_empty());
    }

    #[test]
    fn test_build_environment_section() {
        let cwd = std::path::PathBuf::from("/home/user/project");
        let section = build_environment_section(&cwd);
        assert!(section.contains("/home/user/project"));
        assert!(section.contains("OS:"));
    }
}
