//! Compose a Claude Code-style system prompt.
//!
//! The prompt is built from:
//! - Core behavior guidelines and tool usage instructions
//! - Available tool descriptions
//! - Current working directory, OS info, and date context
//! - Project instructions from CLAUDE.md files

use rust_claude_core::claude_md::ClaudeMdFile;
use rust_claude_tools::ToolRegistry;
use std::path::Path;

/// Maximum total characters for merged CLAUDE.md content.
const CLAUDE_MD_MAX_CHARS: usize = 30_000;

/// Build a complete system prompt suitable for Claude Code-style operation.
pub fn build_system_prompt(
    cwd: &Path,
    tools: &ToolRegistry,
    claude_md_files: &[ClaudeMdFile],
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

    // CLAUDE.md project instructions
    let claude_md_section = build_claude_md_section(claude_md_files);
    if !claude_md_section.is_empty() {
        parts.push(claude_md_section);
    }

    // Custom append (from --append-system-prompt)
    if let Some(append) = custom_append {
        parts.push(append.to_string());
    }

    parts.join("\n\n")
}

const CORE_PROMPT: &str = r#"You are an AI assistant helping with software engineering tasks. You have access to tools for reading files, editing files, writing files, running bash commands, searching for files, searching file contents, and managing todo lists.

# Guidelines

- When asked to modify code, read the relevant file first to understand the context.
- Use the appropriate tool for each task (Bash for commands, FileRead for reading, FileEdit for editing, FileWrite for creating new files, Glob for finding files by pattern, Grep for searching file contents).
- Use Glob to find files by name patterns (e.g., "**/*.rs", "src/**/*.ts") instead of running `find` via Bash.
- Use Grep to search file contents by regex instead of running `grep` or `rg` via Bash.
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

/// Build the `# claudeMd` section from discovered CLAUDE.md files.
///
/// Each file's content is annotated with its source path. If the total content
/// exceeds [`CLAUDE_MD_MAX_CHARS`], files are dropped from the front (global /
/// root-most) to preserve the most specific (leaf-most) project instructions.
fn build_claude_md_section(files: &[ClaudeMdFile]) -> String {
    if files.is_empty() {
        return String::new();
    }

    // Build annotated blocks for each file (leaf-most last).
    let blocks: Vec<String> = files
        .iter()
        .map(|f| {
            format!(
                "Contents of {} (project instructions, checked into the codebase):\n\n{}",
                f.path.display(),
                f.content.trim()
            )
        })
        .collect();

    // Apply truncation: drop from front (global/root) when over budget.
    let mut total: usize = blocks.iter().map(|b| b.len()).sum();
    let mut start_index = 0;

    while total > CLAUDE_MD_MAX_CHARS && start_index < blocks.len() - 1 {
        total -= blocks[start_index].len();
        start_index += 1;
    }

    let mut section = String::from("# claudeMd\n\nCodebase and user instructions are shown below. Be sure to adhere to these instructions.\n");

    if start_index > 0 {
        section.push_str(&format!(
            "\n(Note: {} instruction file(s) truncated due to size limits)\n",
            start_index
        ));
    }

    for (i, block) in blocks[start_index..].iter().enumerate() {
        section.push('\n');
        // If the single remaining block still exceeds the limit, hard-truncate it.
        if block.len() > CLAUDE_MD_MAX_CHARS {
            section.push_str(&block[..CLAUDE_MD_MAX_CHARS]);
            section.push_str("\n\n(truncated due to size limit)");
        } else {
            section.push_str(block);
        }
        if i < blocks[start_index..].len() - 1 {
            section.push('\n');
        }
    }

    section
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
    use std::path::PathBuf;

    #[test]
    fn test_build_system_prompt_contains_core() {
        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(&cwd, &tools, &[], None);
        assert!(prompt.contains("AI assistant"));
        assert!(prompt.contains("software engineering"));
    }

    #[test]
    fn test_build_system_prompt_contains_environment() {
        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(&cwd, &tools, &[], None);
        assert!(prompt.contains("/tmp/test"));
        assert!(prompt.contains("Date:"));
    }

    #[test]
    fn test_build_system_prompt_contains_tools() {
        let cwd = PathBuf::from("/tmp/test");
        let mut tools = ToolRegistry::new();
        tools.register(BashTool::new());
        tools.register(FileReadTool::new());
        let prompt = build_system_prompt(&cwd, &tools, &[], None);
        assert!(prompt.contains("Bash"));
        assert!(prompt.contains("FileRead"));
        assert!(prompt.contains("Available Tools"));
    }

    #[test]
    fn test_build_system_prompt_with_custom_append() {
        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(&cwd, &tools, &[], Some("Custom instructions here"));
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
        let cwd = PathBuf::from("/home/user/project");
        let section = build_environment_section(&cwd);
        assert!(section.contains("/home/user/project"));
        assert!(section.contains("OS:"));
    }

    // --- CLAUDE.md injection tests ---

    #[test]
    fn test_claude_md_section_empty() {
        let section = build_claude_md_section(&[]);
        assert!(section.is_empty());
    }

    #[test]
    fn test_claude_md_section_single_file() {
        let files = vec![ClaudeMdFile {
            path: PathBuf::from("/repo/CLAUDE.md"),
            content: "Use conventional commits.".to_string(),
        }];
        let section = build_claude_md_section(&files);
        assert!(section.contains("# claudeMd"));
        assert!(section.contains("/repo/CLAUDE.md"));
        assert!(section.contains("Use conventional commits."));
    }

    #[test]
    fn test_claude_md_section_multiple_files() {
        let files = vec![
            ClaudeMdFile {
                path: PathBuf::from("/home/user/.claude/CLAUDE.md"),
                content: "Global rules".to_string(),
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.md"),
                content: "Project rules".to_string(),
            },
        ];
        let section = build_claude_md_section(&files);
        assert!(section.contains("Global rules"));
        assert!(section.contains("Project rules"));
        // Global should appear before project
        let global_pos = section.find("Global rules").unwrap();
        let project_pos = section.find("Project rules").unwrap();
        assert!(global_pos < project_pos);
    }

    #[test]
    fn test_claude_md_section_truncation() {
        // Create two files whose combined annotated blocks exceed 30000 chars
        let large_content = "x".repeat(20_000);
        let files = vec![
            ClaudeMdFile {
                path: PathBuf::from("/global/CLAUDE.md"),
                content: large_content.clone(),
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.md"),
                content: large_content.clone(),
            },
        ];
        let section = build_claude_md_section(&files);
        // The global file should be dropped to stay under limit
        assert!(section.contains("truncated"));
        assert!(section.contains("/repo/CLAUDE.md"));
    }

    #[test]
    fn test_claude_md_prompt_ordering() {
        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let files = vec![ClaudeMdFile {
            path: PathBuf::from("/repo/CLAUDE.md"),
            content: "Project instructions here".to_string(),
        }];
        let prompt = build_system_prompt(&cwd, &tools, &files, Some("Custom append"));

        // Verify ordering: environment < claudeMd < custom append
        let env_pos = prompt.find("# Environment").unwrap();
        let claude_md_pos = prompt.find("# claudeMd").unwrap();
        let append_pos = prompt.find("Custom append").unwrap();
        assert!(env_pos < claude_md_pos);
        assert!(claude_md_pos < append_pos);
    }
}
