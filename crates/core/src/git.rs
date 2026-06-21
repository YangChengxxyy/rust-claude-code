use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommitSummary {
    pub short_hash: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitContextSnapshot {
    pub repo_root: PathBuf,
    pub branch: String,
    pub is_clean: bool,
    pub recent_commits: Vec<GitCommitSummary>,
}

pub fn collect_git_context(cwd: &Path) -> Option<GitContextSnapshot> {
    let repo_root = crate::claude_md::find_git_root(cwd)?;
    let branch = run_git(&repo_root, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let status = run_git(&repo_root, &["status", "--short"])?;
    let commits = run_git(&repo_root, &["log", "-5", "--pretty=format:%h %s"])?;

    let recent_commits = commits
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, ' ');
            let short_hash = parts.next()?.trim();
            let subject = parts.next().unwrap_or("").trim();
            if short_hash.is_empty() || subject.is_empty() {
                return None;
            }
            Some(GitCommitSummary {
                short_hash: short_hash.to_string(),
                subject: subject.to_string(),
            })
        })
        .collect();

    Some(GitContextSnapshot {
        repo_root,
        branch,
        is_clean: status.trim().is_empty(),
        recent_commits,
    })
}

fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ── Worktree support (iteration 52) ──
//
// These helpers wrap the `git worktree` plumbing used by the EnterWorktree /
// ExitWorktree tools. State about the active worktree session lives on
// `AppState` (see [`ActiveWorktree`]); the functions here are pure
// side-effecting operations against a repository.

/// Active worktree session metadata, stored on [`crate::state::AppState`].
///
/// `original_cwd` is the working directory the session was rooted in before
/// [`EnterWorktreeTool`] switched it into the worktree; ExitWorktree restores
/// it. `path`/`branch` describe the worktree itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveWorktree {
    pub original_cwd: PathBuf,
    pub path: PathBuf,
    pub branch: String,
}

/// Resolve the git repository root for `cwd`, or `None` if not in a repo.
///
/// Inside a linked worktree this returns the *worktree* directory (which
/// contains the `.git` file), not the main repository root — see
/// [`common_repo_root`] for that.
pub fn repo_root(cwd: &Path) -> Option<PathBuf> {
    crate::claude_md::find_git_root(cwd)
}

/// The main repository root shared by every worktree, resolved from anywhere
/// in the repo. Worktree-management commands (`worktree add/remove`,
/// `branch -d`) are run from here so they never trip over "is current working
/// directory" when the session itself sits inside a linked worktree.
///
/// Returns `None` when `start` is not inside a git repository or git is
/// unavailable.
pub fn common_repo_root(start: &Path) -> Option<PathBuf> {
    let out = run_git_checked(start, &["rev-parse", "--git-common-dir"]).ok()?;
    let raw = PathBuf::from(out.trim());
    let resolved = if raw.is_absolute() {
        raw
    } else {
        start.canonicalize().ok()?.join(raw)
    };
    resolved
        .canonicalize()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
        .filter(|p| p.exists())
}

/// Current short branch name of `cwd`, or `None`.
pub fn current_branch(cwd: &Path) -> Option<String> {
    run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Validate a proposed worktree/branch name. Allows ASCII alphanumerics plus
/// `-_. /`, rejecting empty names, `..`/`.` segments, and leading/trailing
/// slashes — everything that would either create an ambiguous path or a
/// branch name git refuses.
pub fn sanitize_worktree_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("worktree name cannot be empty".to_string());
    }
    if trimmed.starts_with('/') || trimmed.ends_with('/') {
        return Err("worktree name cannot start or end with '/'".to_string());
    }
    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!("invalid worktree name: {trimmed}"));
        }
    }
    let valid = trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'));
    if !valid {
        return Err(format!(
            "invalid worktree name (allowed: letters, digits, - _ . /): {trimmed}"
        ));
    }
    Ok(trimmed.to_string())
}

/// `<repo_root>/.claude/worktrees/<name>` — the conventional worktree layout.
pub fn worktree_dir(repo_root: &Path, name: &str) -> PathBuf {
    repo_root.join(".claude").join("worktrees").join(name)
}

/// Create a new worktree at `<repo_root>/.claude/worktrees/<name>` on a fresh
/// branch of the same name, starting from the current HEAD. Returns the
/// worktree path and branch name.
pub fn create_worktree(repo_root: &Path, name: &str) -> Result<(PathBuf, String), String> {
    let clean = sanitize_worktree_name(name)?;
    let path = worktree_dir(repo_root, &clean);
    if path.exists() {
        return Err(format!(
            "worktree path already exists: {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create .claude/worktrees dir: {e}"))?;
    }
    let path_str = path
        .to_str()
        .ok_or_else(|| "non-UTF8 worktree path".to_string())?;
    run_git_checked(repo_root, &["worktree", "add", "-b", &clean, path_str])
        .map_err(|e| format!("git worktree add failed: {e}"))?;
    Ok((path, clean))
}

/// Validate that `path` is an existing git work tree and return its canonical
/// path plus the checked-out branch name.
pub fn enter_existing_worktree(path: &Path) -> Result<(PathBuf, String), String> {
    let canon = path
        .canonicalize()
        .map_err(|e| format!("worktree path not found: {e}"))?;
    if !is_inside_work_tree(&canon)? {
        return Err(format!("not a git work tree: {}", canon.display()));
    }
    let branch = current_branch(&canon)
        .ok_or_else(|| "could not determine branch of existing worktree".to_string())?;
    Ok((canon, branch))
}

/// True if `path` is inside a git work tree.
pub fn is_inside_work_tree(path: &Path) -> Result<bool, String> {
    match run_git_checked(path, &["rev-parse", "--is-inside-work-tree"]) {
        Ok(out) => Ok(out.trim() == "true"),
        Err(_) => Ok(false),
    }
}

/// True if the worktree at `path` has modified, staged, or untracked files.
pub fn has_uncommitted_changes(path: &Path) -> bool {
    run_git_checked(path, &["status", "--porcelain"])
        .map(|out| !out.trim().is_empty())
        .unwrap_or(false)
}

/// Remove the linked worktree at `path`. With `force`, modified/untracked
/// files are discarded; otherwise git refuses a dirty worktree.
pub fn remove_worktree(repo_root: &Path, path: &Path, force: bool) -> Result<(), String> {
    let path_str = path
        .to_str()
        .ok_or_else(|| "non-UTF8 worktree path".to_string())?;
    let mut args: Vec<&str> = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path_str);
    run_git_checked(repo_root, &args).map_err(|e| format!("git worktree remove failed: {e}"))?;
    Ok(())
}

/// Delete a branch. With `force`, uses `-D` (delete regardless of merge
/// status); otherwise `-d`, which refuses an unmerged branch — the safety net
/// for committed work that [`remove_worktree`] cannot see.
pub fn delete_branch(repo_root: &Path, branch: &str, force: bool) -> Result<(), String> {
    let flag = if force { "-D" } else { "-d" };
    run_git_checked(repo_root, &["branch", flag, branch])
        .map_err(|e| format!("git branch delete failed: {e}"))?;
    Ok(())
}

/// Run git, returning trimmed stdout on success or trimmed stderr on failure.
fn run_git_checked(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to spawn git: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = format!("rust-claude-git-test-{}-{}", name, std::process::id());
        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_collect_git_context_returns_none_outside_repo() {
        let dir = make_temp_dir("outside");
        assert!(collect_git_context(&dir).is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    // ── worktree helpers ──

    use std::process::Command as StdCommand;

    /// Create a temp git repo with one committed file, returning its root.
    /// Worktree branching requires a non-embryonic HEAD, so an initial commit
    /// is mandatory.
    fn make_git_repo(label: &str) -> PathBuf {
        let dir = make_temp_dir(label);
        StdCommand::new("git")
            .args(["init"])
            .current_dir(&dir)
            .output()
            .expect("git init");
        // Per-repo config only (no `--global`): avoids contention on
        // ~/.gitconfig when tests run in parallel, and is authoritative for
        // this throwaway repo.
        for (k, v) in [
            ("user.name", "Test"),
            ("user.email", "test@example.com"),
            ("commit.gpgsign", "false"),
        ] {
            StdCommand::new("git")
                .args(["config", k, v])
                .current_dir(&dir)
                .output()
                .expect("git config");
        }
        fs::write(dir.join("README.md"), "init\n").unwrap();
        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(&dir)
            .output()
            .expect("git add");
        StdCommand::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(&dir)
            .output()
            .expect("git commit");
        dir
    }

    fn git_available() -> bool {
        StdCommand::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn sanitize_accepts_simple_names() {
        assert_eq!(sanitize_worktree_name("fix-123").unwrap(), "fix-123");
        assert_eq!(sanitize_worktree_name("feat_x.y").unwrap(), "feat_x.y");
        assert_eq!(sanitize_worktree_name("a/b").unwrap(), "a/b");
    }

    #[test]
    fn sanitize_rejects_bad_names() {
        assert!(sanitize_worktree_name("").is_err());
        assert!(sanitize_worktree_name("   ").is_err());
        assert!(sanitize_worktree_name("..").is_err());
        assert!(sanitize_worktree_name("a/../b").is_err());
        assert!(sanitize_worktree_name("/lead").is_err());
        assert!(sanitize_worktree_name("trail/").is_err());
        assert!(sanitize_worktree_name("has space").is_err());
        assert!(sanitize_worktree_name("semi;colon").is_err());
    }

    #[test]
    fn worktree_dir_layout() {
        let root = PathBuf::from("/repo");
        assert_eq!(
            worktree_dir(&root, "foo"),
            PathBuf::from("/repo/.claude/worktrees/foo")
        );
    }

    #[test]
    fn create_worktree_makes_dir_and_branch() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("create");
        let (path, branch) = create_worktree(&repo, "topic").expect("create worktree");
        assert!(path.is_dir(), "worktree dir should exist");
        assert_eq!(branch, "topic");
        assert!(path.ends_with(".claude/worktrees/topic"));
        // The worktree should report its own branch as `topic`.
        assert_eq!(current_branch(&path).as_deref(), Some("topic"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn create_worktree_rejects_duplicate_path() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("dup");
        create_worktree(&repo, "wt").unwrap();
        // A second create with the same name must fail (path already exists),
        // not silently clobber the first worktree.
        let err = create_worktree(&repo, "wt").unwrap_err();
        assert!(err.contains("already exists"));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn enter_existing_worktree_resolves_canonical_path_and_branch() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("enter");
        let (wt_path, _) = create_worktree(&repo, "existing").unwrap();
        let (resolved, branch) = enter_existing_worktree(&wt_path).expect("enter existing");
        assert!(resolved.is_absolute());
        assert_eq!(branch, "existing");
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn enter_existing_worktree_rejects_non_repo() {
        let dir = make_temp_dir("not-a-repo");
        let err = enter_existing_worktree(&dir).unwrap_err();
        assert!(err.contains("not a git work tree") || err.contains("not found"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn has_uncommitted_changes_detects_dirty_worktree() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("dirty");
        let (wt_path, _) = create_worktree(&repo, "d").unwrap();
        assert!(!has_uncommitted_changes(&wt_path));
        fs::write(wt_path.join("new.txt"), "untracked\n").unwrap();
        assert!(has_uncommitted_changes(&wt_path));
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn common_repo_root_from_worktree_is_main_root() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("common");
        let canonical_repo = repo.canonicalize().unwrap();
        let (wt_path, _) = create_worktree(&repo, "c").unwrap();
        // From inside the linked worktree, common_repo_root must report the
        // main repository root, not the worktree directory.
        let common = common_repo_root(&wt_path).expect("common repo root");
        assert_eq!(common, canonical_repo);
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn remove_worktree_refuses_dirty_without_force() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("remove-dirty");
        let (wt_path, _) = create_worktree(&repo, "r").unwrap();
        fs::write(wt_path.join("new.txt"), "untracked\n").unwrap();
        let main = common_repo_root(&wt_path).unwrap();
        let err = remove_worktree(&main, &wt_path, false).unwrap_err();
        assert!(err.contains("failed"));
        // Directory must still exist after the refused removal.
        assert!(wt_path.exists());
        let _ = fs::remove_dir_all(&repo);
    }

    #[test]
    fn remove_worktree_and_branch_when_clean() {
        if !git_available() {
            return;
        }
        let repo = make_git_repo("remove-clean");
        let (wt_path, branch) = create_worktree(&repo, "gone").unwrap();
        let main = common_repo_root(&wt_path).unwrap();
        remove_worktree(&main, &wt_path, false).expect("remove worktree");
        assert!(!wt_path.exists());
        delete_branch(&main, &branch, false).expect("delete branch");
        // Branch should no longer exist.
        let branches = run_git(&main, &["branch", "--list"]).unwrap_or_default();
        assert!(!branches.contains("gone"));
        let _ = fs::remove_dir_all(&repo);
    }
}
