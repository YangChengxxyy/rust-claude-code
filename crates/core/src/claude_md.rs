//! CLAUDE.md discovery and loading.
//!
//! Discovers `CLAUDE.md` files by walking up from the current working directory
//! to the git repository root (or filesystem root), plus a global user-level
//! file at `~/.claude/CLAUDE.md` (or `$CLAUDE_CONFIG_DIR/CLAUDE.md`).
//!
//! Also discovers `CLAUDE.local.md` (personal, gitignored) files alongside
//! `CLAUDE.md`, and `.claude/rules/*.md` path-scoped instruction files.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// The type/source of a discovered CLAUDE.md-like file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeMdSourceType {
    /// Global user-level `~/.claude/CLAUDE.md`
    Global,
    /// Global user-level local `~/.claude/CLAUDE.local.md`
    GlobalLocal,
    /// Project-level `CLAUDE.md` (checked into version control)
    Project,
    /// Project-level `CLAUDE.local.md` (personal, not checked in)
    ProjectLocal,
    /// Path-scoped rule file from `.claude/rules/*.md`
    Rule,
}

/// A discovered CLAUDE.md file with its source path and content.
#[derive(Debug, Clone)]
pub struct ClaudeMdFile {
    /// Absolute path to the CLAUDE.md file.
    pub path: PathBuf,
    /// File content (UTF-8).
    pub content: String,
    /// Source type for display and injection annotations.
    pub source_type: ClaudeMdSourceType,
}

/// A discovered rule file from `.claude/rules/` with optional path scope.
#[derive(Debug, Clone)]
pub struct RuleFile {
    /// The underlying CLAUDE.md file entry.
    pub file: ClaudeMdFile,
    /// Glob patterns from YAML frontmatter `paths` field.
    /// If empty, the rule applies everywhere.
    pub paths: Vec<String>,
}

/// Discover all CLAUDE.md files relevant to the given working directory.
///
/// Returns files in merge order: global first (with local variant after),
/// then project files from root-most to leaf-most (CWD) with each directory's
/// local variant immediately after its public file. Path-scoped rule files
/// from `.claude/rules/` appear after all CLAUDE.md/CLAUDE.local.md files.
pub fn discover_claude_md(cwd: &Path) -> Vec<ClaudeMdFile> {
    let mut results = Vec::new();

    // 1. Global CLAUDE.md + CLAUDE.local.md
    let global_dir = global_config_dir();
    if let Some(dir) = &global_dir {
        if let Some(f) = read_claude_md_typed(&dir.join("CLAUDE.md"), ClaudeMdSourceType::Global) {
            results.push(f);
        }
        if let Some(f) = read_claude_md_typed(
            &dir.join("CLAUDE.local.md"),
            ClaudeMdSourceType::GlobalLocal,
        ) {
            results.push(f);
        }
    }

    // 2. Project CLAUDE.md + CLAUDE.local.md files (root-most to leaf-most)
    for dir in project_discovery_dirs(cwd) {
        if let Some(f) = read_claude_md_typed(&dir.join("CLAUDE.md"), ClaudeMdSourceType::Project) {
            results.push(f);
        }
        if let Some(f) = read_claude_md_typed(
            &dir.join("CLAUDE.local.md"),
            ClaudeMdSourceType::ProjectLocal,
        ) {
            results.push(f);
        }
    }

    // 3. Rule files from .claude/rules/ (at project root)
    let rule_files = discover_rule_files(cwd);
    for rf in rule_files {
        results.push(rf.file);
    }

    results
}

/// Discover rule files from `.claude/rules/` at the project root.
/// Returns all rule files with their parsed path patterns.
pub fn discover_rule_files(cwd: &Path) -> Vec<RuleFile> {
    let git_root = find_git_root(cwd);
    let root = match &git_root {
        Some(r) => r.as_path(),
        None => cwd,
    };

    let rules_dir = root.join(".claude").join("rules");
    if !rules_dir.is_dir() {
        return Vec::new();
    }

    let mut rule_files = Vec::new();
    let entries = match std::fs::read_dir(&rules_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut paths: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map_or(false, |ext| ext == "md"))
        .collect();
    paths.sort(); // Deterministic order

    for path in paths {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if content.is_empty() {
                continue;
            }
            let (frontmatter_paths, body) = parse_frontmatter_paths(&content);
            let display_path = path.canonicalize().unwrap_or_else(|_| path.clone());
            rule_files.push(RuleFile {
                file: ClaudeMdFile {
                    path: display_path,
                    content: body,
                    source_type: ClaudeMdSourceType::Rule,
                },
                paths: frontmatter_paths,
            });
        }
    }

    rule_files
}

/// Parse YAML frontmatter `paths` field from markdown content.
/// Returns (paths_list, body_without_frontmatter).
pub fn parse_frontmatter_paths(content: &str) -> (Vec<String>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (Vec::new(), content.to_string());
    }

    // Find the closing ---
    let after_first = &trimmed[3..];
    let rest = after_first.trim_start_matches(|c: char| c == '\r' || c == '\n');
    if let Some(end_pos) = rest.find("\n---") {
        let frontmatter = &rest[..end_pos];
        let body_start = end_pos + 4; // skip "\n---"
        let body = rest[body_start..].trim_start_matches(|c: char| c == '\r' || c == '\n');

        // Parse paths from frontmatter using serde_yaml
        let paths = parse_paths_from_yaml(frontmatter);
        (paths, body.to_string())
    } else {
        // No closing ---, treat as no frontmatter
        (Vec::new(), content.to_string())
    }
}

/// Extract `paths` array from YAML frontmatter string.
fn parse_paths_from_yaml(yaml: &str) -> Vec<String> {
    // Use serde_yaml to parse
    let value: Result<serde_yaml::Value, _> = serde_yaml::from_str(yaml);
    match value {
        Ok(serde_yaml::Value::Mapping(map)) => {
            if let Some(paths_val) = map.get(&serde_yaml::Value::String("paths".into())) {
                if let serde_yaml::Value::Sequence(seq) = paths_val {
                    return seq
                        .iter()
                        .filter_map(|v| {
                            if let serde_yaml::Value::String(s) = v {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                }
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

/// Get the global config directory path.
fn global_config_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        Some(PathBuf::from(dir))
    } else {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".claude"))
    }
}

/// Discover the global user-level CLAUDE.md file (legacy, used by tests).
#[cfg(test)]
fn discover_global_claude_md() -> Option<ClaudeMdFile> {
    let dir = global_config_dir()?;
    let path = dir.join("CLAUDE.md");
    read_claude_md_typed(&path, ClaudeMdSourceType::Global)
}

/// Discover project-level CLAUDE.md files by walking up from `cwd` (legacy).
#[cfg(test)]
fn discover_project_claude_md(cwd: &Path) -> Vec<ClaudeMdFile> {
    project_discovery_dirs(cwd)
        .into_iter()
        .filter_map(|dir| read_claude_md_typed(&dir.join("CLAUDE.md"), ClaudeMdSourceType::Project))
        .collect()
}

/// Return canonicalized directories to inspect for project-scoped files.
///
/// The order is root-most to leaf-most and the walk stops at the git root
/// (inclusive) when one exists.
pub fn project_discovery_dirs(cwd: &Path) -> Vec<PathBuf> {
    let cwd = match cwd.canonicalize() {
        Ok(p) => p,
        Err(_) => cwd.to_path_buf(),
    };

    let git_root = find_git_root(&cwd);
    let stop_at = git_root.as_deref();

    let mut dirs = Vec::new();
    let mut visited = HashSet::new();
    let mut current = Some(cwd.as_path());

    while let Some(dir) = current {
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        if !visited.insert(canonical.clone()) {
            break;
        }

        dirs.push(canonical);

        if let Some(root) = stop_at {
            if dir == root {
                break;
            }
        }

        current = dir.parent();
    }

    dirs.reverse();
    dirs
}

/// Find the git repository root by walking up from `start`.
///
/// Returns the directory containing `.git` (file or directory), or `None` if
/// not inside a git repository.
pub fn find_git_root(start: &Path) -> Option<PathBuf> {
    let canonical = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    let mut current = Some(canonical.as_path());
    while let Some(dir) = current {
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

/// Try to read a CLAUDE.md file with a source type. Returns `None` if the file
/// does not exist or cannot be read (e.g., permission denied).
fn read_claude_md_typed(path: &Path, source_type: ClaudeMdSourceType) -> Option<ClaudeMdFile> {
    match std::fs::read_to_string(path) {
        Ok(content) if !content.is_empty() => {
            let display_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            Some(ClaudeMdFile {
                path: display_path,
                content,
                source_type,
            })
        }
        _ => None,
    }
}

/// Legacy read helper (for backward compatibility in tests).
#[cfg(test)]
fn read_claude_md(path: &Path) -> Option<ClaudeMdFile> {
    read_claude_md_typed(path, ClaudeMdSourceType::Project)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = format!("rust-claude-md-test-{}-{}", name, std::process::id());
        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_no_claude_md_returns_empty() {
        let dir = make_temp_dir("empty");
        let results = discover_project_claude_md(&dir);
        assert!(results.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_single_claude_md_in_cwd() {
        let dir = make_temp_dir("single");
        fs::write(dir.join("CLAUDE.md"), "# Project Rules\nUse Rust.").unwrap();

        let results = discover_project_claude_md(&dir);
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("Use Rust"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_claude_md_in_parent_and_child() {
        let parent = make_temp_dir("parent-child");
        let child = parent.join("sub");
        fs::create_dir_all(&child).unwrap();

        fs::write(parent.join("CLAUDE.md"), "Parent rules").unwrap();
        fs::write(child.join("CLAUDE.md"), "Child rules").unwrap();

        let results = discover_project_claude_md(&child);
        assert!(results.len() >= 2);
        assert!(results[results.len() - 2].content.contains("Parent rules"));
        assert!(results[results.len() - 1].content.contains("Child rules"));
        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn test_stops_at_git_root() {
        let root = make_temp_dir("git-root");
        let git_dir = root.join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let sub = root.join("a").join("b");
        fs::create_dir_all(&sub).unwrap();

        fs::write(root.join("CLAUDE.md"), "Repo root").unwrap();
        fs::write(sub.join("CLAUDE.md"), "Sub dir").unwrap();

        let results = discover_project_claude_md(&sub);
        assert_eq!(results.len(), 2);
        assert!(results[0].content.contains("Repo root"));
        assert!(results[1].content.contains("Sub dir"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_empty_claude_md_skipped() {
        let dir = make_temp_dir("empty-file");
        fs::write(dir.join("CLAUDE.md"), "").unwrap();

        let results = discover_project_claude_md(&dir);
        assert!(results.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_git_root() {
        let root = make_temp_dir("git-find");
        fs::create_dir_all(root.join(".git")).unwrap();
        let sub = root.join("deep").join("path");
        fs::create_dir_all(&sub).unwrap();

        let found = find_git_root(&sub);
        assert!(found.is_some());
        assert_eq!(
            found.unwrap().canonicalize().unwrap(),
            root.canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_project_discovery_dirs_use_root_to_leaf_order() {
        let root = make_temp_dir("dirs");
        fs::create_dir_all(root.join(".git")).unwrap();
        let leaf = root.join("packages/app");
        fs::create_dir_all(&leaf).unwrap();

        let dirs = project_discovery_dirs(&leaf);
        assert_eq!(
            dirs.first().unwrap().canonicalize().unwrap(),
            root.canonicalize().unwrap()
        );
        assert_eq!(
            dirs.last().unwrap().canonicalize().unwrap(),
            leaf.canonicalize().unwrap()
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_discover_claude_md_ordering() {
        let root = make_temp_dir("ordering");
        fs::create_dir_all(root.join(".git")).unwrap();

        let mid = root.join("packages");
        let leaf = mid.join("app");
        fs::create_dir_all(&leaf).unwrap();

        fs::write(root.join("CLAUDE.md"), "root").unwrap();
        fs::write(mid.join("CLAUDE.md"), "mid").unwrap();
        fs::write(leaf.join("CLAUDE.md"), "leaf").unwrap();

        let results = discover_project_claude_md(&leaf);
        assert_eq!(results.len(), 3);
        assert!(results[0].content.contains("root"));
        assert!(results[1].content.contains("mid"));
        assert!(results[2].content.contains("leaf"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_global_claude_md_with_config_dir() {
        let dir = make_temp_dir("global-config");
        fs::write(dir.join("CLAUDE.md"), "Global rules").unwrap();

        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", dir.to_str().unwrap());
        }
        let result = discover_global_claude_md();
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }

        assert!(result.is_some());
        assert!(result.unwrap().content.contains("Global rules"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_full_discover_with_global() {
        let config_dir = make_temp_dir("full-global");
        let project = make_temp_dir("full-project");
        fs::create_dir_all(project.join(".git")).unwrap();

        fs::write(config_dir.join("CLAUDE.md"), "Global").unwrap();
        fs::write(project.join("CLAUDE.md"), "Project").unwrap();

        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", config_dir.to_str().unwrap());
        }
        let results = discover_claude_md(&project);
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }

        assert!(results.len() >= 2);
        assert!(results[0].content.contains("Global"));
        assert!(results.iter().any(|f| f.content.contains("Project")));

        let _ = fs::remove_dir_all(&config_dir);
        let _ = fs::remove_dir_all(&project);
    }

    // --- CLAUDE.local.md tests ---

    #[test]
    fn test_local_md_next_to_claude_md() {
        let config_dir = make_temp_dir("local-next");
        let project = make_temp_dir("local-next-proj");
        fs::create_dir_all(project.join(".git")).unwrap();

        fs::write(project.join("CLAUDE.md"), "Project shared").unwrap();
        fs::write(project.join("CLAUDE.local.md"), "Project local").unwrap();

        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", config_dir.to_str().unwrap());
        }
        let results = discover_claude_md(&project);
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }

        // CLAUDE.md should come before CLAUDE.local.md
        let shared_idx = results
            .iter()
            .position(|f| f.content.contains("Project shared"))
            .unwrap();
        let local_idx = results
            .iter()
            .position(|f| f.content.contains("Project local"))
            .unwrap();
        assert_eq!(local_idx, shared_idx + 1);
        assert_eq!(results[shared_idx].source_type, ClaudeMdSourceType::Project);
        assert_eq!(
            results[local_idx].source_type,
            ClaudeMdSourceType::ProjectLocal
        );

        let _ = fs::remove_dir_all(&config_dir);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn test_local_md_without_claude_md() {
        let config_dir = make_temp_dir("local-only-cfg");
        let project = make_temp_dir("local-only-proj");
        fs::create_dir_all(project.join(".git")).unwrap();

        // No CLAUDE.md, only CLAUDE.local.md
        fs::write(project.join("CLAUDE.local.md"), "Local only").unwrap();

        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", config_dir.to_str().unwrap());
        }
        let results = discover_claude_md(&project);
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }

        assert!(results.iter().any(|f| f.content.contains("Local only")));

        let _ = fs::remove_dir_all(&config_dir);
        let _ = fs::remove_dir_all(&project);
    }

    #[test]
    fn test_global_local_md() {
        let config_dir = make_temp_dir("global-local");
        let project = make_temp_dir("global-local-proj");
        fs::create_dir_all(project.join(".git")).unwrap();

        fs::write(config_dir.join("CLAUDE.md"), "Global shared").unwrap();
        fs::write(config_dir.join("CLAUDE.local.md"), "Global local").unwrap();

        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", config_dir.to_str().unwrap());
        }
        let results = discover_claude_md(&project);
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }

        let global_idx = results
            .iter()
            .position(|f| f.content.contains("Global shared"))
            .unwrap();
        let global_local_idx = results
            .iter()
            .position(|f| f.content.contains("Global local"))
            .unwrap();
        assert_eq!(global_local_idx, global_idx + 1);
        assert_eq!(results[global_idx].source_type, ClaudeMdSourceType::Global);
        assert_eq!(
            results[global_local_idx].source_type,
            ClaudeMdSourceType::GlobalLocal
        );

        let _ = fs::remove_dir_all(&config_dir);
        let _ = fs::remove_dir_all(&project);
    }

    // --- Rules discovery tests ---

    #[test]
    fn test_rules_discovery_multiple_files() {
        let root = make_temp_dir("rules-multi");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude/rules")).unwrap();

        fs::write(root.join(".claude/rules/frontend.md"), "Frontend rules").unwrap();
        fs::write(root.join(".claude/rules/backend.md"), "Backend rules").unwrap();
        fs::write(root.join(".claude/rules/general.md"), "General rules").unwrap();

        let rule_files = discover_rule_files(&root);
        assert_eq!(rule_files.len(), 3);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_rules_discovery_no_rules_dir() {
        let root = make_temp_dir("rules-nodir");
        fs::create_dir_all(root.join(".git")).unwrap();

        let rule_files = discover_rule_files(&root);
        assert!(rule_files.is_empty());

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_rules_in_full_discovery() {
        let config_dir = make_temp_dir("rules-full-cfg");
        let root = make_temp_dir("rules-full-proj");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude/rules")).unwrap();

        fs::write(root.join("CLAUDE.md"), "Project").unwrap();
        fs::write(root.join("CLAUDE.local.md"), "Project local").unwrap();
        fs::write(root.join(".claude/rules/style.md"), "Style rules").unwrap();

        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", config_dir.to_str().unwrap());
        }
        let results = discover_claude_md(&root);
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }

        // Rules should come after CLAUDE.md/CLAUDE.local.md
        let project_idx = results
            .iter()
            .position(|f| f.content.contains("Project local"))
            .unwrap();
        let rule_idx = results
            .iter()
            .position(|f| f.content.contains("Style rules"))
            .unwrap();
        assert!(rule_idx > project_idx);
        assert_eq!(results[rule_idx].source_type, ClaudeMdSourceType::Rule);

        let _ = fs::remove_dir_all(&config_dir);
        let _ = fs::remove_dir_all(&root);
    }

    // --- Frontmatter parsing tests ---

    #[test]
    fn test_parse_frontmatter_with_paths() {
        let content =
            "---\npaths:\n  - \"src/frontend/**\"\n  - \"src/shared/**\"\n---\nFrontend rules here";
        let (paths, body) = parse_frontmatter_paths(content);
        assert_eq!(paths, vec!["src/frontend/**", "src/shared/**"]);
        assert_eq!(body, "Frontend rules here");
    }

    #[test]
    fn test_parse_frontmatter_no_paths() {
        let content = "---\ntitle: General\n---\nGeneral rules";
        let (paths, body) = parse_frontmatter_paths(content);
        assert!(paths.is_empty());
        assert_eq!(body, "General rules");
    }

    #[test]
    fn test_parse_frontmatter_none() {
        let content = "# Just markdown\nNo frontmatter here";
        let (paths, body) = parse_frontmatter_paths(content);
        assert!(paths.is_empty());
        assert_eq!(body, content);
    }

    #[test]
    fn test_rules_with_frontmatter_paths() {
        let root = make_temp_dir("rules-fm");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude/rules")).unwrap();

        fs::write(
            root.join(".claude/rules/frontend.md"),
            "---\npaths:\n  - \"src/frontend/**\"\n---\nFrontend rules",
        )
        .unwrap();

        let rule_files = discover_rule_files(&root);
        assert_eq!(rule_files.len(), 1);
        assert_eq!(rule_files[0].paths, vec!["src/frontend/**"]);
        assert_eq!(rule_files[0].file.content, "Frontend rules");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_manual_verification_local_and_rule_files_are_discovered() {
        let config_dir = make_temp_dir("manual-context-cfg");
        let root = make_temp_dir("manual-context-proj");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join(".claude/rules")).unwrap();

        fs::write(root.join("CLAUDE.md"), "Project shared").unwrap();
        fs::write(root.join("CLAUDE.local.md"), "Project local").unwrap();
        fs::write(
            root.join(".claude/rules/test.md"),
            "---\npaths:\n  - \"**\"\n---\nRule text",
        )
        .unwrap();

        unsafe {
            std::env::set_var("CLAUDE_CONFIG_DIR", config_dir.to_str().unwrap());
        }
        let results = discover_claude_md(&root);
        unsafe {
            std::env::remove_var("CLAUDE_CONFIG_DIR");
        }

        assert!(results.iter().any(|f| f.path.ends_with("CLAUDE.local.md")));
        assert!(results
            .iter()
            .any(|f| f.path.ends_with(".claude/rules/test.md")));

        let local_idx = results
            .iter()
            .position(|f| f.path.ends_with("CLAUDE.local.md"))
            .unwrap();
        let rule_idx = results
            .iter()
            .position(|f| f.path.ends_with(".claude/rules/test.md"))
            .unwrap();
        assert!(rule_idx > local_idx);

        let _ = fs::remove_dir_all(&config_dir);
        let _ = fs::remove_dir_all(&root);
    }
}
