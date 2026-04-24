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
    let output = Command::new("git").args(args).current_dir(cwd).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
}
