//! CLAUDE.md discovery and loading.
//!
//! Discovers `CLAUDE.md` files by walking up from the current working directory
//! to the git repository root (or filesystem root), plus a global user-level
//! file at `~/.claude/CLAUDE.md` (or `$CLAUDE_CONFIG_DIR/CLAUDE.md`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A discovered CLAUDE.md file with its source path and content.
#[derive(Debug, Clone)]
pub struct ClaudeMdFile {
    /// Absolute path to the CLAUDE.md file.
    pub path: PathBuf,
    /// File content (UTF-8).
    pub content: String,
}

/// Discover all CLAUDE.md files relevant to the given working directory.
///
/// Returns files in merge order: global first, then project files from
/// root-most to leaf-most (CWD).
pub fn discover_claude_md(cwd: &Path) -> Vec<ClaudeMdFile> {
    let mut results = Vec::new();

    // 1. Global CLAUDE.md
    if let Some(global) = discover_global_claude_md() {
        results.push(global);
    }

    // 2. Project CLAUDE.md files (root-most to leaf-most)
    let project_files = discover_project_claude_md(cwd);
    results.extend(project_files);

    results
}

/// Discover the global user-level CLAUDE.md file.
///
/// Checks `$CLAUDE_CONFIG_DIR/CLAUDE.md` or `~/.claude/CLAUDE.md`.
fn discover_global_claude_md() -> Option<ClaudeMdFile> {
    let config_dir = if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
        PathBuf::from(dir)
    } else {
        let home = std::env::var("HOME").ok()?;
        PathBuf::from(home).join(".claude")
    };

    let path = config_dir.join("CLAUDE.md");
    read_claude_md(&path)
}

/// Discover project-level CLAUDE.md files by walking up from `cwd`.
///
/// Stops at the git repository root (directory containing `.git`) or the
/// filesystem root. Returns files ordered from root-most to leaf-most.
fn discover_project_claude_md(cwd: &Path) -> Vec<ClaudeMdFile> {
    project_discovery_dirs(cwd)
        .into_iter()
        .filter_map(|dir| read_claude_md(&dir.join("CLAUDE.md")))
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

/// Try to read a CLAUDE.md file. Returns `None` if the file does not exist
/// or cannot be read (e.g., permission denied).
fn read_claude_md(path: &Path) -> Option<ClaudeMdFile> {
    match std::fs::read_to_string(path) {
        Ok(content) if !content.is_empty() => {
            let display_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            Some(ClaudeMdFile {
                path: display_path,
                content,
            })
        }
        _ => None,
    }
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
}
