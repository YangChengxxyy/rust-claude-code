use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Trust status for a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustStatus {
    /// Directory is explicitly trusted (has an entry in trust.json).
    Trusted,
    /// Directory is not trusted and no parent directory is trusted.
    Untrusted,
    /// Directory inherits trust from a parent directory.
    InheritedFromParent,
}

impl TrustStatus {
    /// Returns true if the directory is trusted (either explicitly or inherited).
    pub fn is_trusted(&self) -> bool {
        matches!(self, TrustStatus::Trusted | TrustStatus::InheritedFromParent)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustRecord {
    trusted_at: String,
}

/// Manages directory trust state for security gating of `apiKeyHelper` and
/// `settings.json` env variables.
///
/// Trust state is persisted to `~/.config/rust-claude-code/trust.json`.
/// The home directory is a special case: trust is only held in memory for the
/// current process lifetime and never persisted.
#[derive(Debug)]
pub struct TrustManager {
    /// Persisted trust records loaded from trust.json.
    records: HashMap<PathBuf, TrustRecord>,
    /// Path to the trust.json file.
    trust_file: PathBuf,
    /// In-memory trust for the home directory (not persisted).
    home_trusted: bool,
}

impl TrustManager {
    /// Create a new TrustManager, loading existing trust state from disk.
    pub fn new() -> Self {
        let trust_file = Self::trust_file_path();
        let records = Self::load_records(&trust_file);
        TrustManager {
            records,
            trust_file,
            home_trusted: false,
        }
    }

    /// Create a TrustManager with a custom trust file path (for testing).
    #[cfg(test)]
    fn with_trust_file(trust_file: PathBuf) -> Self {
        let records = Self::load_records(&trust_file);
        TrustManager {
            records,
            trust_file,
            home_trusted: false,
        }
    }

    /// Check whether a directory is trusted.
    pub fn check_trust(&self, project_dir: &Path) -> TrustStatus {
        let canonical = match project_dir.canonicalize() {
            Ok(p) => p,
            Err(_) => project_dir.to_path_buf(),
        };

        // Check home directory in-memory trust
        if let Some(home) = Self::home_dir() {
            if canonical == home && self.home_trusted {
                return TrustStatus::Trusted;
            }
        }

        // Check exact match
        if self.records.contains_key(&canonical) {
            return TrustStatus::Trusted;
        }

        // Check parent inheritance
        let mut ancestor = canonical.parent();
        while let Some(parent) = ancestor {
            // Home directory in-memory trust also cascades
            if let Some(ref home) = Self::home_dir() {
                if parent == home.as_path() && self.home_trusted {
                    return TrustStatus::InheritedFromParent;
                }
            }

            if self.records.contains_key(parent) {
                return TrustStatus::InheritedFromParent;
            }
            ancestor = parent.parent();
        }

        TrustStatus::Untrusted
    }

    /// Accept trust for a directory. Persists to trust.json unless the
    /// directory is the home directory (in which case trust is in-memory only).
    pub fn accept_trust(&mut self, project_dir: &Path) -> Result<(), std::io::Error> {
        let canonical = match project_dir.canonicalize() {
            Ok(p) => p,
            Err(_) => project_dir.to_path_buf(),
        };

        // Home directory special handling: in-memory only
        if let Some(home) = Self::home_dir() {
            if canonical == home {
                self.home_trusted = true;
                return Ok(());
            }
        }

        let record = TrustRecord {
            trusted_at: chrono::Utc::now().to_rfc3339(),
        };
        self.records.insert(canonical, record);
        self.save_records()
    }

    fn trust_file_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("rust-claude-code")
            .join("trust.json")
    }

    fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }

    fn load_records(path: &Path) -> HashMap<PathBuf, TrustRecord> {
        if !path.exists() {
            return HashMap::new();
        }
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    fn save_records(&self) -> Result<(), std::io::Error> {
        if let Some(parent) = self.trust_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.records)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&self.trust_file, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir(name: &str) -> PathBuf {
        let unique = format!("trust-test-{}-{}", name, std::process::id());
        let path = std::env::temp_dir().join(unique);
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn test_untrusted_by_default() {
        let temp = make_temp_dir("untrusted");
        let trust_file = temp.join("trust.json");
        let manager = TrustManager::with_trust_file(trust_file);

        let status = manager.check_trust(&temp.join("some-project"));
        assert_eq!(status, TrustStatus::Untrusted);
        assert!(!status.is_trusted());
    }

    #[test]
    fn test_explicit_trust() {
        let temp = make_temp_dir("explicit");
        let trust_file = temp.join("trust.json");
        let project = temp.join("my-project");
        fs::create_dir_all(&project).unwrap();

        let mut manager = TrustManager::with_trust_file(trust_file.clone());
        manager.accept_trust(&project).unwrap();

        let status = manager.check_trust(&project);
        assert_eq!(status, TrustStatus::Trusted);
        assert!(status.is_trusted());

        // Verify persistence: new manager loads from disk
        let manager2 = TrustManager::with_trust_file(trust_file);
        assert_eq!(manager2.check_trust(&project), TrustStatus::Trusted);
    }

    #[test]
    fn test_parent_inheritance() {
        let temp = make_temp_dir("inheritance");
        let trust_file = temp.join("trust.json");
        let parent_dir = temp.join("workspace");
        let child_dir = parent_dir.join("sub-project");
        fs::create_dir_all(&child_dir).unwrap();

        let mut manager = TrustManager::with_trust_file(trust_file);
        manager.accept_trust(&parent_dir).unwrap();

        assert_eq!(
            manager.check_trust(&parent_dir),
            TrustStatus::Trusted
        );
        assert_eq!(
            manager.check_trust(&child_dir),
            TrustStatus::InheritedFromParent
        );
        assert!(manager.check_trust(&child_dir).is_trusted());
    }

    #[test]
    fn test_sibling_not_trusted() {
        let temp = make_temp_dir("sibling");
        let trust_file = temp.join("trust.json");
        let project_a = temp.join("project-a");
        let project_b = temp.join("project-b");
        fs::create_dir_all(&project_a).unwrap();
        fs::create_dir_all(&project_b).unwrap();

        let mut manager = TrustManager::with_trust_file(trust_file);
        manager.accept_trust(&project_a).unwrap();

        assert_eq!(manager.check_trust(&project_a), TrustStatus::Trusted);
        assert_eq!(manager.check_trust(&project_b), TrustStatus::Untrusted);
    }

    #[test]
    fn test_home_directory_in_memory_only() {
        let temp = make_temp_dir("home");
        let trust_file = temp.join("trust.json");
        // We can't easily test the real home directory, so we test the
        // in-memory flag mechanism directly.
        let mut manager = TrustManager::with_trust_file(trust_file.clone());
        manager.home_trusted = true;

        // Check that home trust is not persisted
        assert!(manager.records.is_empty());

        // New manager does not have home trust
        let manager2 = TrustManager::with_trust_file(trust_file);
        assert!(!manager2.home_trusted);
    }

    #[test]
    fn test_trust_json_roundtrip() {
        let temp = make_temp_dir("roundtrip");
        let trust_file = temp.join("trust.json");
        let project = temp.join("my-project");
        fs::create_dir_all(&project).unwrap();

        let mut manager = TrustManager::with_trust_file(trust_file.clone());
        manager.accept_trust(&project).unwrap();

        // Verify file contents
        let content = fs::read_to_string(&trust_file).unwrap();
        let parsed: HashMap<String, serde_json::Value> =
            serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.len(), 1);
        let canonical = project.canonicalize().unwrap();
        assert!(parsed.contains_key(&canonical.to_string_lossy().to_string()));
    }
}
