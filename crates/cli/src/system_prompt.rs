//! Compose a Claude Code-style system prompt.
//!
//! The prompt is built from:
//! - Core behavior guidelines and tool usage instructions
//! - Available tool descriptions
//! - Current working directory, OS info, and date context
//! - Project instructions from CLAUDE.md files
//! - Git context for the current repository when available

use rust_claude_core::claude_md::{ClaudeMdFile, ClaudeMdSourceType};
use rust_claude_core::git::GitContextSnapshot;
use rust_claude_core::memory::{
    build_memory_contract_prompt, build_relevant_memories_section, RelevantMemory,
    ScannedMemoryStore,
};
use rust_claude_tools::ToolRegistry;
use std::path::Path;

/// Maximum total characters for merged CLAUDE.md content.
const CLAUDE_MD_MAX_CHARS: usize = 30_000;

/// Build a complete system prompt suitable for Claude Code-style operation.
pub fn build_system_prompt(
    cwd: &Path,
    tools: &ToolRegistry,
    claude_md_files: &[ClaudeMdFile],
    memory_store: Option<&ScannedMemoryStore>,
    relevant_memories: &[RelevantMemory],
    git_context: Option<&GitContextSnapshot>,
    custom_append: Option<&str>,
) -> String {
    let mut parts = Vec::new();

    parts.push(CORE_PROMPT.to_string());

    let tool_section = build_tool_section(tools);
    if !tool_section.is_empty() {
        parts.push(tool_section);
    }

    parts.push(build_environment_section(cwd));

    if let Some(memory_section) = build_memory_section(memory_store) {
        parts.push(memory_section);
    }

    if let Some(relevant_memory_section) = build_relevant_memories_section(relevant_memories) {
        parts.push(relevant_memory_section);
    }

    if let Some(git_section) = build_git_context_section(git_context) {
        parts.push(git_section);
    }

    let claude_md_section = build_claude_md_section(claude_md_files);
    if !claude_md_section.is_empty() {
        parts.push(claude_md_section);
    }

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

fn build_memory_section(memory_store: Option<&ScannedMemoryStore>) -> Option<String> {
    let memory = memory_store?;
    let mut lines = vec![
        build_memory_contract_prompt(),
        String::new(),
        "# memoryStore".to_string(),
        String::new(),
    ];
    lines.push(format!(
        "- memory_dir: {}",
        memory.store.memory_dir.display()
    ));
    lines.push(format!(
        "- entrypoint: {}",
        memory.store.entrypoint.display()
    ));
    lines.push(format!("- entry_count: {}", memory.entries.len()));

    if let Some(index) = &memory.index {
        lines.push(String::new());
        lines.push(format!(
            "Contents of {} (user auto-memory entrypoint):\n\n{}{}",
            index.path.display(),
            index.content.trim(),
            if index.truncated {
                "\n\n(truncated due to size limits)"
            } else {
                ""
            }
        ));
    }

    Some(lines.join("\n"))
}

/// Annotation text for a CLAUDE.md file based on its source type.
fn source_annotation(file: &ClaudeMdFile) -> &'static str {
    match file.source_type {
        ClaudeMdSourceType::Global => "user instructions, global",
        ClaudeMdSourceType::GlobalLocal => {
            "local user instructions, not checked into version control"
        }
        ClaudeMdSourceType::Project => "project instructions, checked into the codebase",
        ClaudeMdSourceType::ProjectLocal => {
            "local project instructions, not checked into version control"
        }
        ClaudeMdSourceType::Rule => "project rules",
    }
}

/// Truncation priority category. Lower number = truncated first.
fn truncation_priority(file: &ClaudeMdFile) -> u8 {
    match file.source_type {
        ClaudeMdSourceType::Global | ClaudeMdSourceType::GlobalLocal => 0,
        ClaudeMdSourceType::Rule => 1,
        // Root-level project files truncated before deeper ones, but that is
        // handled by iterating from the start of the block list (which is
        // ordered root-to-leaf).
        ClaudeMdSourceType::Project | ClaudeMdSourceType::ProjectLocal => 2,
    }
}

fn build_claude_md_section(files: &[ClaudeMdFile]) -> String {
    if files.is_empty() {
        return String::new();
    }

    let blocks: Vec<String> = files
        .iter()
        .map(|f| {
            format!(
                "Contents of {} ({}):\n\n{}",
                f.path.display(),
                source_annotation(f),
                f.content.trim()
            )
        })
        .collect();

    // Build a truncation order: indices sorted by truncation priority (lowest first).
    // Within the same priority, root-most (earlier index) is truncated first.
    let mut truncation_order: Vec<usize> = (0..files.len()).collect();
    truncation_order.sort_by_key(|&i| (truncation_priority(&files[i]), i));

    let mut total: usize = blocks.iter().map(|b| b.len()).sum();
    let mut included = vec![true; blocks.len()];
    let mut truncated_count = 0;

    for &idx in &truncation_order {
        if total <= CLAUDE_MD_MAX_CHARS {
            break;
        }
        // Don't truncate the very last included block
        let remaining_included = included.iter().filter(|&&b| b).count();
        if remaining_included <= 1 {
            break;
        }
        total -= blocks[idx].len();
        included[idx] = false;
        truncated_count += 1;
    }

    let mut section = String::from("# claudeMd\n\nCodebase and user instructions are shown below. Be sure to adhere to these instructions.\n");

    if truncated_count > 0 {
        section.push_str(&format!(
            "\n(Note: {} instruction file(s) truncated due to size limits)\n",
            truncated_count
        ));
    }

    let mut first = true;
    for (i, block) in blocks.iter().enumerate() {
        if !included[i] {
            continue;
        }
        section.push('\n');
        if block.len() > CLAUDE_MD_MAX_CHARS {
            section.push_str(&block[..CLAUDE_MD_MAX_CHARS]);
            section.push_str("\n\n(truncated due to size limit)");
        } else {
            section.push_str(block);
        }
        if first {
            first = false;
        } else {
            // Separator between blocks already handled by the \n above
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

fn build_git_context_section(git_context: Option<&GitContextSnapshot>) -> Option<String> {
    let git = git_context?;
    let commits = if git.recent_commits.is_empty() {
        "- none".to_string()
    } else {
        git.recent_commits
            .iter()
            .map(|commit| format!("- {} {}", commit.short_hash, commit.subject))
            .collect::<Vec<_>>()
            .join("\n")
    };

    Some(format!(
        "# gitStatus\n\n- repo_root: {}\n- branch: {}\n- clean: {}\n- recent_commits:\n{}",
        git.repo_root.display(),
        git.branch,
        git.is_clean,
        commits,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::git::{GitCommitSummary, GitContextSnapshot};
    use rust_claude_tools::{BashTool, FileReadTool};
    use std::path::PathBuf;

    #[test]
    fn test_build_system_prompt_contains_core() {
        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(&cwd, &tools, &[], None, &[], None, None);
        assert!(prompt.contains("AI assistant"));
        assert!(prompt.contains("software engineering"));
    }

    #[test]
    fn test_build_system_prompt_contains_environment() {
        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(&cwd, &tools, &[], None, &[], None, None);
        assert!(prompt.contains("/tmp/test"));
        assert!(prompt.contains("Date:"));
    }

    #[test]
    fn test_build_system_prompt_contains_tools() {
        let cwd = PathBuf::from("/tmp/test");
        let mut tools = ToolRegistry::new();
        tools.register(BashTool::new());
        tools.register(FileReadTool::new());
        let prompt = build_system_prompt(&cwd, &tools, &[], None, &[], None, None);
        assert!(prompt.contains("Bash"));
        assert!(prompt.contains("FileRead"));
        assert!(prompt.contains("Available Tools"));
    }

    #[test]
    fn test_build_system_prompt_with_custom_append() {
        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let prompt = build_system_prompt(
            &cwd,
            &tools,
            &[],
            None,
            &[],
            None,
            Some("Custom instructions here"),
        );
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
            source_type: ClaudeMdSourceType::Project,
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
                source_type: ClaudeMdSourceType::Global,
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.md"),
                content: "Project rules".to_string(),
                source_type: ClaudeMdSourceType::Project,
            },
        ];
        let section = build_claude_md_section(&files);
        assert!(section.contains("Global rules"));
        assert!(section.contains("Project rules"));
        let global_pos = section.find("Global rules").unwrap();
        let project_pos = section.find("Project rules").unwrap();
        assert!(global_pos < project_pos);
    }

    #[test]
    fn test_claude_md_section_truncation() {
        let large_content = "x".repeat(20_000);
        let files = vec![
            ClaudeMdFile {
                path: PathBuf::from("/global/CLAUDE.md"),
                content: large_content.clone(),
                source_type: ClaudeMdSourceType::Global,
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.md"),
                content: large_content.clone(),
                source_type: ClaudeMdSourceType::Project,
            },
        ];
        let section = build_claude_md_section(&files);
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
            source_type: ClaudeMdSourceType::Project,
        }];
        let prompt =
            build_system_prompt(&cwd, &tools, &files, None, &[], None, Some("Custom append"));

        let env_pos = prompt.find("# Environment").unwrap();
        let claude_md_pos = prompt.find("# claudeMd").unwrap();
        let append_pos = prompt.find("Custom append").unwrap();
        assert!(env_pos < claude_md_pos);
        assert!(claude_md_pos < append_pos);
    }

    #[test]
    fn test_git_context_section_is_included() {
        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let git_context = GitContextSnapshot {
            repo_root: PathBuf::from("/repo"),
            branch: "main".into(),
            is_clean: true,
            recent_commits: vec![GitCommitSummary {
                short_hash: "abc123".into(),
                subject: "Initial commit".into(),
            }],
        };
        let prompt = build_system_prompt(&cwd, &tools, &[], None, &[], Some(&git_context), None);
        assert!(prompt.contains("# gitStatus"));
        assert!(prompt.contains("branch: main"));
        assert!(prompt.contains("abc123 Initial commit"));
    }

    #[test]
    fn test_memory_section_is_included() {
        use rust_claude_core::memory::{MemoryIndex, MemoryStore, ScannedMemoryStore};

        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let memory = ScannedMemoryStore {
            store: MemoryStore {
                project_root: PathBuf::from("/repo"),
                memory_dir: PathBuf::from("/home/user/.claude/projects/repo/memory"),
                entrypoint: PathBuf::from("/home/user/.claude/projects/repo/memory/MEMORY.md"),
            },
            index: Some(MemoryIndex {
                path: PathBuf::from("/home/user/.claude/projects/repo/memory/MEMORY.md"),
                content: "- [Testing](feedback_testing.md) - use real DB".to_string(),
                truncated: false,
            }),
            entries: vec![],
        };

        let prompt = build_system_prompt(&cwd, &tools, &[], Some(&memory), &[], None, None);
        assert!(prompt.contains("# memoryContract"));
        assert!(prompt.contains("# memoryStore"));
        assert!(prompt.contains("feedback_testing.md"));
    }

    #[test]
    fn test_relevant_memory_section_is_included() {
        use rust_claude_core::memory::{MemoryType, RelevantMemory};

        let cwd = PathBuf::from("/tmp/test");
        let tools = ToolRegistry::new();
        let relevant = vec![RelevantMemory {
            relative_path: "testing.md".to_string(),
            memory_type: Some(MemoryType::Feedback),
            description: Some("Use real DB in tests".to_string()),
            freshness_days: 3,
            body: "Use real database integration tests.".to_string(),
        }];

        let prompt = build_system_prompt(&cwd, &tools, &[], None, &relevant, None, None);
        assert!(prompt.contains("# relevantMemories"));
        assert!(prompt.contains("testing.md"));
        assert!(prompt.contains("freshness_days: 3"));
    }

    // --- Local file annotation tests ---

    #[test]
    fn test_claude_md_local_file_annotation() {
        let files = vec![
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.md"),
                content: "Shared instructions".to_string(),
                source_type: ClaudeMdSourceType::Project,
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.local.md"),
                content: "Local instructions".to_string(),
                source_type: ClaudeMdSourceType::ProjectLocal,
            },
        ];
        let section = build_claude_md_section(&files);
        assert!(section.contains("project instructions, checked into the codebase"));
        assert!(section.contains("local project instructions, not checked into version control"));
    }

    #[test]
    fn test_claude_md_rule_file_annotation() {
        let files = vec![
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.md"),
                content: "Project".to_string(),
                source_type: ClaudeMdSourceType::Project,
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/.claude/rules/testing.md"),
                content: "Testing rules".to_string(),
                source_type: ClaudeMdSourceType::Rule,
            },
        ];
        let section = build_claude_md_section(&files);
        assert!(section.contains("project rules"));
        assert!(section.contains("Testing rules"));
    }

    #[test]
    fn test_claude_md_rule_file_after_local() {
        let files = vec![
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.md"),
                content: "Project".to_string(),
                source_type: ClaudeMdSourceType::Project,
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.local.md"),
                content: "Local".to_string(),
                source_type: ClaudeMdSourceType::ProjectLocal,
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/.claude/rules/style.md"),
                content: "Style rules".to_string(),
                source_type: ClaudeMdSourceType::Rule,
            },
        ];
        let section = build_claude_md_section(&files);
        let local_pos = section.find("Local").unwrap();
        let rules_pos = section.find("Style rules").unwrap();
        assert!(rules_pos > local_pos);
    }

    #[test]
    fn test_truncation_with_mixed_types() {
        // Global should be truncated first, then rules, then project
        let large = "x".repeat(15_000);
        let files = vec![
            ClaudeMdFile {
                path: PathBuf::from("/home/.claude/CLAUDE.md"),
                content: large.clone(),
                source_type: ClaudeMdSourceType::Global,
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/CLAUDE.md"),
                content: large.clone(),
                source_type: ClaudeMdSourceType::Project,
            },
            ClaudeMdFile {
                path: PathBuf::from("/repo/.claude/rules/style.md"),
                content: large.clone(),
                source_type: ClaudeMdSourceType::Rule,
            },
        ];
        let section = build_claude_md_section(&files);
        // Global should be truncated first (priority 0), then rule (priority 1)
        // Project should remain
        assert!(section.contains("truncated"));
        assert!(section.contains("/repo/CLAUDE.md"));
    }
}
