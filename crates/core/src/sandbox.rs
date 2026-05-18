use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    #[default]
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub network: NetworkPolicy,
    #[serde(default)]
    pub allowed_paths: Vec<PathBuf>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            network: NetworkPolicy::Allow,
            allowed_paths: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxAdapterAvailability {
    Available { name: &'static str },
    Unavailable { reason: String },
}

impl SandboxAdapterAvailability {
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available { .. })
    }
}

pub fn canonicalize_allowed_paths(
    paths: &[PathBuf],
    cwd: &Path,
    home: Option<&Path>,
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        let expanded = expand_path(path, cwd, home);
        let canonical = expanded.canonicalize().unwrap_or(expanded);
        if !out.contains(&canonical) {
            out.push(canonical);
        }
    }
    out
}

pub fn default_allowed_paths(cwd: &Path) -> Vec<PathBuf> {
    canonicalize_allowed_paths(&[cwd.to_path_buf()], cwd, home_dir().as_deref())
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn expand_path(path: &Path, cwd: &Path, home: Option<&Path>) -> PathBuf {
    let raw = path.as_os_str().to_string_lossy();
    if raw == "~" {
        return home
            .map(Path::to_path_buf)
            .unwrap_or_else(|| cwd.join(path));
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = home {
            return home.join(rest);
        }
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_defaults_are_disabled() {
        let config = SandboxConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.network, NetworkPolicy::Allow);
        assert!(config.allowed_paths.is_empty());
    }

    #[test]
    fn allowed_paths_resolve_relative_and_home() {
        let cwd = std::env::temp_dir();
        let home = cwd.join("home");
        let paths = vec![PathBuf::from("."), PathBuf::from("~/project")];
        let resolved = canonicalize_allowed_paths(&paths, &cwd, Some(&home));

        assert_eq!(resolved[0], cwd.canonicalize().unwrap_or(cwd.clone()));
        assert_eq!(resolved[1], home.join("project"));
    }
}
