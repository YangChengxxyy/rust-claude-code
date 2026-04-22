use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::RegexBuilder;
use rust_claude_core::tool_types::{ToolInfo, ToolResult};
use walkdir::WalkDir;

use crate::tool::{Tool, ToolContext, ToolError};

#[derive(Debug, Clone, Default)]
pub struct GrepTool;

#[derive(Debug, Clone, serde::Deserialize)]
struct GrepInput {
    pattern: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    glob: Option<String>,
    #[serde(default, rename = "type")]
    file_type: Option<String>,
    #[serde(default)]
    output_mode: Option<String>,
    #[serde(default)]
    head_limit: Option<usize>,
    #[serde(default, rename = "-A")]
    after_context: Option<usize>,
    #[serde(default, rename = "-B")]
    before_context: Option<usize>,
    #[serde(default, rename = "-C")]
    context: Option<usize>,
    #[serde(default, rename = "-i")]
    case_insensitive: Option<bool>,
}

/// Map file type shorthand to extensions.
fn type_to_extensions(file_type: &str) -> Option<Vec<&'static str>> {
    match file_type {
        "rs" => Some(vec!["rs"]),
        "py" => Some(vec!["py", "pyi"]),
        "js" => Some(vec!["js", "mjs", "cjs"]),
        "ts" => Some(vec!["ts", "mts", "cts"]),
        "tsx" => Some(vec!["tsx"]),
        "jsx" => Some(vec!["jsx"]),
        "go" => Some(vec!["go"]),
        "java" => Some(vec!["java"]),
        "c" => Some(vec!["c", "h"]),
        "cpp" => Some(vec!["cpp", "cc", "cxx", "hpp", "hh", "hxx", "h"]),
        "rb" => Some(vec!["rb"]),
        "php" => Some(vec!["php"]),
        "swift" => Some(vec!["swift"]),
        "kt" => Some(vec!["kt", "kts"]),
        "scala" => Some(vec!["scala"]),
        "sh" => Some(vec!["sh", "bash", "zsh"]),
        "css" => Some(vec!["css"]),
        "html" => Some(vec!["html", "htm"]),
        "json" => Some(vec!["json"]),
        "yaml" | "yml" => Some(vec!["yaml", "yml"]),
        "toml" => Some(vec!["toml"]),
        "xml" => Some(vec!["xml"]),
        "md" => Some(vec!["md", "markdown"]),
        "sql" => Some(vec!["sql"]),
        "r" => Some(vec!["r", "R"]),
        "lua" => Some(vec!["lua"]),
        "zig" => Some(vec!["zig"]),
        "dart" => Some(vec!["dart"]),
        "ex" | "elixir" => Some(vec!["ex", "exs"]),
        "erl" | "erlang" => Some(vec!["erl", "hrl"]),
        _ => None,
    }
}

/// Check if a file extension matches the given type shorthand.
fn matches_file_type(path: &Path, file_type: &str) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e,
        None => return false,
    };
    match type_to_extensions(file_type) {
        Some(exts) => exts.contains(&ext),
        None => ext == file_type,
    }
}

/// Check if a file name matches the given glob pattern.
fn matches_glob_filter(path: &Path, glob_pattern: &str) -> bool {
    let pattern = match glob::Pattern::new(glob_pattern) {
        Ok(p) => p,
        Err(_) => return false,
    };
    // Match against file name and the full path
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if pattern.matches(name) {
            return true;
        }
    }
    if let Some(s) = path.to_str() {
        if pattern.matches(s) {
            return true;
        }
    }
    false
}

/// Check if a file is likely binary by reading a small prefix.
fn is_likely_binary(path: &Path) -> bool {
    use std::fs::File;
    use std::io::Read;
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return true,
    };
    let mut buf = [0u8; 512];
    let n = match file.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return true,
    };
    buf[..n].contains(&0)
}

impl GrepTool {
    pub fn new() -> Self {
        Self
    }

    fn collect_files(
        search_root: &Path,
        glob_filter: Option<&str>,
        file_type: Option<&str>,
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for entry in WalkDir::new(search_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| {
                // Skip hidden directories
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.') || e.depth() == 0
            })
        {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().to_path_buf();

            // Apply file type filter
            if let Some(ft) = file_type {
                if !matches_file_type(&path, ft) {
                    continue;
                }
            }

            // Apply glob filter
            if let Some(gf) = glob_filter {
                if !matches_glob_filter(&path, gf) {
                    continue;
                }
            }

            // Skip binary files
            if is_likely_binary(&path) {
                continue;
            }

            files.push(path);
        }
        files
    }

    fn search_files_with_matches(
        files: &[PathBuf],
        re: &regex::Regex,
        head_limit: usize,
    ) -> String {
        let mut matched_paths = Vec::new();
        for path in files {
            if matched_paths.len() >= head_limit {
                break;
            }
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if re.is_match(&content) {
                matched_paths.push(path.to_string_lossy().to_string());
            }
        }
        matched_paths.join("\n")
    }

    fn search_content(
        files: &[PathBuf],
        re: &regex::Regex,
        head_limit: usize,
        before_ctx: usize,
        after_ctx: usize,
    ) -> String {
        let mut output_lines = Vec::new();

        for path in files {
            if output_lines.len() >= head_limit {
                break;
            }
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let lines: Vec<&str> = content.lines().collect();
            let path_str = path.to_string_lossy();

            // Collect line indices that match
            let mut match_indices: BTreeSet<usize> = BTreeSet::new();
            for (i, line) in lines.iter().enumerate() {
                if re.is_match(line) {
                    match_indices.insert(i);
                }
            }

            if match_indices.is_empty() {
                continue;
            }

            // Expand context around matches
            let mut visible: BTreeSet<usize> = BTreeSet::new();
            for &idx in &match_indices {
                let start = idx.saturating_sub(before_ctx);
                let end = (idx + after_ctx).min(lines.len().saturating_sub(1));
                for i in start..=end {
                    visible.insert(i);
                }
            }

            for &i in &visible {
                if output_lines.len() >= head_limit {
                    break;
                }
                output_lines.push(format!("{}:{}:{}", path_str, i + 1, lines[i]));
            }
        }

        output_lines.join("\n")
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn info(&self) -> ToolInfo {
        ToolInfo {
            name: "Grep".to_string(),
            description: "Search file contents using regex patterns".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory to search in. Defaults to current working directory."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g., \"*.rs\", \"**/*.ts\")"
                    },
                    "type": {
                        "type": "string",
                        "description": "File type shorthand (e.g., \"rs\", \"py\", \"js\", \"go\")"
                    },
                    "output_mode": {
                        "type": "string",
                        "enum": ["files_with_matches", "content"],
                        "description": "Output mode: files_with_matches (default) or content"
                    },
                    "head_limit": {
                        "type": "integer",
                        "description": "Max output entries (default 250)"
                    },
                    "-A": {
                        "type": "integer",
                        "description": "Lines to show after each match"
                    },
                    "-B": {
                        "type": "integer",
                        "description": "Lines to show before each match"
                    },
                    "-C": {
                        "type": "integer",
                        "description": "Context lines (before and after)"
                    },
                    "-i": {
                        "type": "boolean",
                        "description": "Case-insensitive search"
                    }
                },
                "required": ["pattern"]
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
        let input: GrepInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(e.to_string()))?;

        let case_insensitive = input.case_insensitive.unwrap_or(false);
        let re = RegexBuilder::new(&input.pattern)
            .case_insensitive(case_insensitive)
            .build()
            .map_err(|e| ToolError::InvalidInput(format!("invalid regex: {e}")))?;

        let search_root = match &input.path {
            Some(p) => PathBuf::from(p),
            None => std::env::current_dir()
                .map_err(|e| ToolError::Execution(format!("cannot get cwd: {e}")))?,
        };

        // If path is a file, search just that file
        let files = if search_root.is_file() {
            vec![search_root]
        } else {
            Self::collect_files(
                &search_root,
                input.glob.as_deref(),
                input.file_type.as_deref(),
            )
        };

        let head_limit = input.head_limit.unwrap_or(250);
        let output_mode = input.output_mode.as_deref().unwrap_or("files_with_matches");

        let output = match output_mode {
            "content" => {
                let before_ctx = input.context.or(input.before_context).unwrap_or(0);
                let after_ctx = input.context.or(input.after_context).unwrap_or(0);
                Self::search_content(&files, &re, head_limit, before_ctx, after_ctx)
            }
            _ => Self::search_files_with_matches(&files, &re, head_limit),
        };

        Ok(ToolResult::success(context.tool_use_id, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    async fn make_temp_dir(suffix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("grep-tool-{}-{}", std::process::id(), suffix));
        let _ = fs::remove_dir_all(&dir).await;
        fs::create_dir_all(&dir).await.unwrap();
        dir
    }

    fn ctx() -> ToolContext {
        ToolContext {
            tool_use_id: "tool_1".to_string(),
            app_state: None,
                    agent_context: None,
        }
    }

    async fn setup_test_files(dir: &Path) {
        fs::write(dir.join("main.rs"), "fn main() {\n    println!(\"hello\");\n}\n")
            .await
            .unwrap();
        fs::write(dir.join("lib.rs"), "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n")
            .await
            .unwrap();
        fs::write(dir.join("notes.txt"), "TODO: fix the bug\nDone: refactor\n")
            .await
            .unwrap();
        fs::create_dir_all(dir.join("sub")).await.unwrap();
        fs::write(dir.join("sub/util.rs"), "pub fn helper() {}\n")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_grep_basic_search() {
        let dir = make_temp_dir("basic").await;
        setup_test_files(&dir).await;

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({ "pattern": "fn main", "path": dir.to_str().unwrap() }),
                ctx(),
            )
            .await
            .unwrap();

        assert!(result.content.contains("main.rs"));
        assert!(!result.content.contains("lib.rs"));
    }

    #[tokio::test]
    async fn test_grep_regex_pattern() {
        let dir = make_temp_dir("regex").await;
        setup_test_files(&dir).await;

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "pub fn \\w+",
                    "path": dir.to_str().unwrap()
                }),
                ctx(),
            )
            .await
            .unwrap();

        // Should match lib.rs and sub/util.rs
        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[tokio::test]
    async fn test_grep_files_with_matches_mode() {
        let dir = make_temp_dir("fwm").await;
        setup_test_files(&dir).await;

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "fn",
                    "path": dir.to_str().unwrap(),
                    "output_mode": "files_with_matches"
                }),
                ctx(),
            )
            .await
            .unwrap();

        // main.rs, lib.rs, sub/util.rs all contain "fn"
        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[tokio::test]
    async fn test_grep_content_mode() {
        let dir = make_temp_dir("content").await;
        setup_test_files(&dir).await;

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "fn main",
                    "path": dir.to_str().unwrap(),
                    "output_mode": "content"
                }),
                ctx(),
            )
            .await
            .unwrap();

        assert!(result.content.contains("main.rs:1:fn main()"));
    }

    #[tokio::test]
    async fn test_grep_context_lines() {
        let dir = make_temp_dir("ctx").await;
        setup_test_files(&dir).await;

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "println",
                    "path": dir.to_str().unwrap(),
                    "output_mode": "content",
                    "-C": 1
                }),
                ctx(),
            )
            .await
            .unwrap();

        let lines: Vec<&str> = result.content.lines().collect();
        // Should include the match line plus 1 before and 1 after
        assert!(lines.len() >= 3, "expected context lines, got: {:?}", lines);
        assert!(lines.iter().any(|l| l.contains("fn main")));
        assert!(lines.iter().any(|l| l.contains("println")));
    }

    #[tokio::test]
    async fn test_grep_file_type_filter() {
        let dir = make_temp_dir("type").await;
        setup_test_files(&dir).await;

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "fn",
                    "path": dir.to_str().unwrap(),
                    "type": "rs"
                }),
                ctx(),
            )
            .await
            .unwrap();

        // Only .rs files should match
        let lines: Vec<&str> = result.content.lines().collect();
        assert!(lines.iter().all(|l| l.ends_with(".rs")));
        assert!(!result.content.contains("notes.txt"));
    }

    #[tokio::test]
    async fn test_grep_glob_filter() {
        let dir = make_temp_dir("glob").await;
        setup_test_files(&dir).await;

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "fn",
                    "path": dir.to_str().unwrap(),
                    "glob": "main.*"
                }),
                ctx(),
            )
            .await
            .unwrap();

        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines.len(), 1);
        assert!(result.content.contains("main.rs"));
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let dir = make_temp_dir("case").await;
        fs::write(dir.join("test.txt"), "Error\nerror\nERROR\nfine\n")
            .await
            .unwrap();

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "error",
                    "path": dir.to_str().unwrap(),
                    "output_mode": "content",
                    "-i": true
                }),
                ctx(),
            )
            .await
            .unwrap();

        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines.len(), 3);
    }

    #[tokio::test]
    async fn test_grep_head_limit() {
        let dir = make_temp_dir("limit").await;
        let mut content = String::new();
        for i in 0..100 {
            content.push_str(&format!("line {i} match\n"));
        }
        fs::write(dir.join("big.txt"), &content).await.unwrap();

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "match",
                    "path": dir.to_str().unwrap(),
                    "output_mode": "content",
                    "head_limit": 5
                }),
                ctx(),
            )
            .await
            .unwrap();

        let lines: Vec<&str> = result.content.lines().collect();
        assert_eq!(lines.len(), 5);
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = make_temp_dir("nomatch").await;
        fs::write(dir.join("test.txt"), "hello world\n").await.unwrap();

        let tool = GrepTool::new();
        let result = tool
            .execute(
                serde_json::json!({
                    "pattern": "zzz_nonexistent",
                    "path": dir.to_str().unwrap()
                }),
                ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.content, "");
    }

    #[tokio::test]
    async fn test_grep_missing_pattern() {
        let tool = GrepTool::new();
        let result = tool.execute(serde_json::json!({}), ctx()).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidInput(_) => {}
            other => panic!("expected InvalidInput, got: {other}"),
        }
    }

    #[test]
    fn test_type_to_extensions() {
        assert_eq!(type_to_extensions("rs"), Some(vec!["rs"]));
        assert_eq!(type_to_extensions("py"), Some(vec!["py", "pyi"]));
        assert_eq!(type_to_extensions("js"), Some(vec!["js", "mjs", "cjs"]));
        assert!(type_to_extensions("nonexistent").is_none());
    }

    #[test]
    fn test_matches_file_type() {
        assert!(matches_file_type(Path::new("foo.rs"), "rs"));
        assert!(!matches_file_type(Path::new("foo.py"), "rs"));
        assert!(matches_file_type(Path::new("foo.mjs"), "js"));
    }
}
