use rust_claude_core::mcp_config::McpServersConfig;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// A plugin declaration file (`plugin.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub mcp_servers: McpServersConfig,
    #[serde(default)]
    pub custom_agents: Vec<PluginAgentDefinition>,
    #[serde(default)]
    pub slash_commands: Vec<PluginSlashCommand>,
}

/// A custom agent declared in a plugin manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginAgentDefinition {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// A slash command declared in a plugin manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginSlashCommand {
    pub name: String,
    pub description: String,
    /// The prompt template. `{args}` is replaced with the command arguments.
    pub prompt: String,
}

/// A loaded plugin instance with resolved paths.
#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub name: String,
    pub version: String,
    pub description: String,
    pub manifest_path: std::path::PathBuf,
    pub mcp_servers: McpServersConfig,
    pub custom_agents: Vec<PluginAgentDefinition>,
    pub slash_commands: Vec<PluginSlashCommand>,
    /// Whether this plugin came from the project directory (higher priority).
    pub is_project: bool,
}

/// Error during plugin loading.
#[derive(Debug, thiserror::Error)]
pub enum PluginLoadError {
    #[error("invalid manifest in {path}: {message}")]
    InvalidManifest { path: PathBuf, message: String },
    #[error("duplicate plugin name '{name}' at {path}; already loaded from {existing}")]
    DuplicateName {
        name: String,
        path: PathBuf,
        existing: PathBuf,
    },
    #[error("plugin path does not exist: {0}")]
    PathNotFound(PathBuf),
    #[error("plugin path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("plugin manifest is missing: {0}")]
    MissingManifest(PathBuf),
    #[error("plugin install target already exists: {0}")]
    TargetExists(PathBuf),
    #[error("plugin '{0}' is not installed in the user plugin directory")]
    NotUserInstalled(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Discovers and loads plugins from user and project directories.
pub struct PluginLoader {
    plugins: Vec<LoadedPlugin>,
}

impl PluginLoader {
    /// Discover plugins from `~/.claude/plugins/` and optionally `.claude/plugins/`.
    /// Project plugins take precedence over user plugins with the same name.
    pub fn discover(project_dir: Option<&Path>) -> Self {
        let mut plugins = Vec::new();

        // Scan user plugins first
        if let Some(user_dir) = user_plugins_dir() {
            let mut user_plugins = scan_plugins_dir(&user_dir, false);
            plugins.append(&mut user_plugins);
        }

        // Scan project plugins (higher priority)
        if let Some(project_dir) = project_dir {
            let project_plugins_dir = project_dir.join(".claude").join("plugins");
            if project_plugins_dir.is_dir() {
                let mut project_plugins = scan_plugins_dir(&project_plugins_dir, true);
                for p in &project_plugins {
                    // Remove user plugin with the same name (project overrides)
                    plugins.retain(|existing| existing.name != p.name);
                }
                plugins.append(&mut project_plugins);
            }
        }

        PluginLoader { plugins }
    }

    /// Returns discovered plugins without loading (for listing).
    pub fn discovered(&self) -> &[LoadedPlugin] {
        &self.plugins
    }

    /// Unload all plugins (placeholder — resource cleanup is done by callers).
    pub fn unload(&mut self) {
        self.plugins.clear();
    }
}

/// Get the user plugins directory.
fn user_plugins_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok().or_else(|| {
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERPROFILE").ok()
        }
        #[cfg(not(target_os = "windows"))]
        {
            None
        }
    })?;
    Some(Path::new(&home).join(".claude").join("plugins"))
}

/// Scan a plugins directory for subdirectories containing plugin.json.
fn scan_plugins_dir(dir: &Path, is_project: bool) -> Vec<LoadedPlugin> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut plugins = Vec::new();
    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }
        let manifest_path = plugin_dir.join("plugin.json");
        if !manifest_path.is_file() {
            continue;
        }

        let content = match std::fs::read_to_string(&manifest_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Warning: cannot read {}: {}", manifest_path.display(), e);
                continue;
            }
        };

        let manifest: PluginManifest = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "Warning: invalid plugin manifest {}: {}",
                    manifest_path.display(),
                    e
                );
                continue;
            }
        };

        plugins.push(LoadedPlugin {
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
            manifest_path,
            mcp_servers: manifest.mcp_servers,
            custom_agents: manifest.custom_agents,
            slash_commands: manifest.slash_commands,
            is_project,
        });
    }

    plugins
}

/// Manages plugin lifecycle: discovery, loading, and integration.
pub struct PluginManager {
    loader: PluginLoader,
}

impl PluginManager {
    pub fn new(project_dir: Option<&Path>) -> Self {
        Self {
            loader: PluginLoader::discover(project_dir),
        }
    }

    pub fn plugins(&self) -> &[LoadedPlugin] {
        self.loader.discovered()
    }

    pub fn reload(&mut self, project_dir: Option<&Path>) {
        self.loader.unload();
        self.loader = PluginLoader::discover(project_dir);
    }

    pub fn install_from_path(
        &mut self,
        source: &Path,
        project_dir: Option<&Path>,
    ) -> Result<LoadedPlugin, PluginLoadError> {
        if !source.exists() {
            return Err(PluginLoadError::PathNotFound(source.to_path_buf()));
        }
        if !source.is_dir() {
            return Err(PluginLoadError::NotDirectory(source.to_path_buf()));
        }

        let manifest_path = source.join("plugin.json");
        if !manifest_path.is_file() {
            return Err(PluginLoadError::MissingManifest(manifest_path));
        }

        let manifest = read_manifest(&manifest_path)?;
        validate_manifest(&manifest, &manifest_path)?;
        let user_dir = user_plugins_dir().ok_or_else(|| PluginLoadError::InvalidManifest {
            path: manifest_path.clone(),
            message: "could not resolve user plugin directory".to_string(),
        })?;
        std::fs::create_dir_all(&user_dir)?;
        let target = user_dir.join(&manifest.name);
        if target.exists() {
            return Err(PluginLoadError::TargetExists(target));
        }

        copy_dir_recursive(source, &target)?;
        self.reload(project_dir);

        self.plugins()
            .iter()
            .find(|plugin| !plugin.is_project && plugin.name == manifest.name)
            .cloned()
            .ok_or_else(|| PluginLoadError::InvalidManifest {
                path: target.join("plugin.json"),
                message: "installed plugin was not discovered after reload".to_string(),
            })
    }

    pub fn remove_installed(
        &mut self,
        name: &str,
        project_dir: Option<&Path>,
    ) -> Result<(), PluginLoadError> {
        let user_dir = user_plugins_dir()
            .ok_or_else(|| PluginLoadError::NotUserInstalled(name.to_string()))?;
        let target = user_dir.join(name);
        if !target.exists() {
            return Err(PluginLoadError::NotUserInstalled(name.to_string()));
        }
        if !target.is_dir() {
            return Err(PluginLoadError::NotDirectory(target));
        }

        std::fs::remove_dir_all(&target)?;
        self.reload(project_dir);
        Ok(())
    }

    pub fn unload_all(&mut self) {
        self.loader.unload();
    }
}

fn read_manifest(path: &Path) -> Result<PluginManifest, PluginLoadError> {
    let content = std::fs::read_to_string(path)?;
    serde_json::from_str(&content).map_err(|error| PluginLoadError::InvalidManifest {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn validate_manifest(manifest: &PluginManifest, path: &Path) -> Result<(), PluginLoadError> {
    if manifest.name.trim().is_empty() {
        return Err(PluginLoadError::InvalidManifest {
            path: path.to_path_buf(),
            message: "plugin name cannot be empty".to_string(),
        });
    }
    if manifest.name.contains('/') || manifest.name.contains('\\') || manifest.name.contains("..") {
        return Err(PluginLoadError::InvalidManifest {
            path: path.to_path_buf(),
            message: "plugin name must be a simple directory name".to_string(),
        });
    }
    if manifest.version.trim().is_empty() {
        return Err(PluginLoadError::InvalidManifest {
            path: path.to_path_buf(),
            message: "plugin version cannot be empty".to_string(),
        });
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            std::fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn test_parse_minimal_manifest() {
        let json = r#"{
            "name": "my-plugin",
            "version": "1.0.0"
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "my-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "");
        assert!(manifest.mcp_servers.is_empty());
        assert!(manifest.custom_agents.is_empty());
        assert!(manifest.slash_commands.is_empty());
    }

    #[test]
    fn test_parse_full_manifest() {
        let json = r#"{
            "name": "my-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "mcp_servers": {
                "test-server": {
                    "transport_type": "stdio",
                    "command": "node",
                    "args": ["server.js"]
                }
            },
            "custom_agents": [{
                "name": "reviewer",
                "description": "Reviews code",
                "system_prompt": "You are a code reviewer.",
                "tools": ["FileRead", "Glob"],
                "model": "claude-sonnet-4-20250514"
            }],
            "slash_commands": [{
                "name": "/deploy",
                "description": "Deploy to prod",
                "prompt": "Deploy this project to production"
            }]
        }"#;
        let manifest: PluginManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name, "my-plugin");
        assert_eq!(manifest.description, "A test plugin");
        assert_eq!(manifest.mcp_servers.len(), 1);
        assert_eq!(manifest.custom_agents.len(), 1);
        assert_eq!(manifest.custom_agents[0].name, "reviewer");
        assert_eq!(manifest.slash_commands.len(), 1);
        assert_eq!(manifest.slash_commands[0].name, "/deploy");
    }

    #[test]
    fn test_reject_missing_name() {
        let json = r#"{"version": "1.0.0"}"#;
        let result: Result<PluginManifest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_reject_missing_version() {
        let json = r#"{"name": "my-plugin"}"#;
        let result: Result<PluginManifest, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_discover_empty_dir() {
        let dir =
            std::env::temp_dir().join(format!("rust-claude-plugin-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let home = dir.join("home");
        with_home(&home, || {
            let loader = PluginLoader::discover(Some(&dir));
            assert!(loader.discovered().is_empty());
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_discover_with_invalid_manifest() {
        let dir =
            std::env::temp_dir().join(format!("rust-claude-plugin-invalid-{}", std::process::id()));
        let plugins_dir = dir.join(".claude").join("plugins").join("bad-plugin");
        std::fs::create_dir_all(&plugins_dir).unwrap();
        std::fs::write(plugins_dir.join("plugin.json"), "not json").unwrap();

        let home = dir.join("home");
        with_home(&home, || {
            let loader = PluginLoader::discover(Some(&dir));
            // Invalid manifests are skipped
            assert!(loader.discovered().is_empty());
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_project_overrides_user() {
        let _guard = env_lock();
        let tmp = std::env::temp_dir().join(format!(
            "rust-claude-plugin-override-{}",
            std::process::id()
        ));
        // Create user plugin
        let user_dir = tmp.join("user-plugins");
        let user_plugin = user_dir.join("shared").join("plugin.json");
        std::fs::create_dir_all(user_plugin.parent().unwrap()).unwrap();
        std::fs::write(
            &user_plugin,
            r#"{"name":"shared","version":"1.0.0","description":"user"}"#,
        )
        .unwrap();

        // Create project plugin (same name)
        let project_dir = tmp.join("project-dir");
        let project_plugin = project_dir
            .join(".claude")
            .join("plugins")
            .join("shared")
            .join("plugin.json");
        std::fs::create_dir_all(project_plugin.parent().unwrap()).unwrap();
        std::fs::write(
            &project_plugin,
            r#"{"name":"shared","version":"2.0.0","description":"project"}"#,
        )
        .unwrap();

        // We need to override HOME for the test
        let old_home = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", user_dir.to_str().unwrap()) };

        let loader = PluginLoader::discover(Some(&project_dir));

        // Restore HOME
        if let Some(h) = old_home {
            unsafe { std::env::set_var("HOME", h) };
        } else {
            unsafe { std::env::remove_var("HOME") };
        }

        let plugins = loader.discovered();
        assert_eq!(
            plugins.len(),
            1,
            "should have 1 plugin (project overrides user)"
        );
        assert_eq!(plugins[0].name, "shared");
        assert_eq!(plugins[0].version, "2.0.0"); // project version wins
        assert_eq!(plugins[0].description, "project");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rust-claude-plugin-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn with_home<T>(home: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = env_lock();
        let old_home = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", home) };
        let result = f();
        match old_home {
            Some(value) => unsafe { std::env::set_var("HOME", value) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        result
    }

    fn write_plugin(dir: &Path, name: &str, version: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join("plugin.json"),
            format!(r#"{{"name":"{name}","version":"{version}","description":"test"}}"#),
        )
        .unwrap();
    }

    #[test]
    fn install_from_path_copies_and_reloads_plugin() {
        let tmp = unique_temp_dir("install");
        let home = tmp.join("home");
        let source = tmp.join("source");
        write_plugin(&source, "my-plugin", "1.0.0");

        with_home(&home, || {
            let mut manager = PluginManager::new(None);
            let plugin = manager.install_from_path(&source, None).unwrap();
            assert_eq!(plugin.name, "my-plugin");
            assert!(home
                .join(".claude")
                .join("plugins")
                .join("my-plugin")
                .join("plugin.json")
                .is_file());
            assert_eq!(manager.plugins().len(), 1);
        });

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_from_missing_path_returns_error() {
        let tmp = unique_temp_dir("missing");
        let home = tmp.join("home");
        with_home(&home, || {
            let mut manager = PluginManager::new(None);
            let error = manager
                .install_from_path(&tmp.join("missing"), None)
                .unwrap_err();
            assert!(matches!(error, PluginLoadError::PathNotFound(_)));
        });
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_missing_manifest_returns_error() {
        let tmp = unique_temp_dir("missing-manifest");
        let home = tmp.join("home");
        let source = tmp.join("source");
        std::fs::create_dir_all(&source).unwrap();
        with_home(&home, || {
            let mut manager = PluginManager::new(None);
            let error = manager.install_from_path(&source, None).unwrap_err();
            assert!(matches!(error, PluginLoadError::MissingManifest(_)));
        });
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_invalid_version_returns_error() {
        let tmp = unique_temp_dir("invalid-version");
        let home = tmp.join("home");
        let source = tmp.join("source");
        write_plugin(&source, "my-plugin", "");
        with_home(&home, || {
            let mut manager = PluginManager::new(None);
            let error = manager.install_from_path(&source, None).unwrap_err();
            assert!(matches!(error, PluginLoadError::InvalidManifest { .. }));
        });
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn install_existing_target_returns_conflict() {
        let tmp = unique_temp_dir("target-exists");
        let home = tmp.join("home");
        let source = tmp.join("source");
        write_plugin(&source, "my-plugin", "1.0.0");
        with_home(&home, || {
            let target = home.join(".claude").join("plugins").join("my-plugin");
            write_plugin(&target, "my-plugin", "0.1.0");
            let mut manager = PluginManager::new(None);
            let error = manager.install_from_path(&source, None).unwrap_err();
            assert!(matches!(error, PluginLoadError::TargetExists(_)));
        });
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn remove_installed_plugin_deletes_user_directory() {
        let tmp = unique_temp_dir("remove");
        let home = tmp.join("home");
        with_home(&home, || {
            let target = home.join(".claude").join("plugins").join("my-plugin");
            write_plugin(&target, "my-plugin", "1.0.0");
            let mut manager = PluginManager::new(None);
            assert_eq!(manager.plugins().len(), 1);
            manager.remove_installed("my-plugin", None).unwrap();
            assert!(!target.exists());
            assert!(manager.plugins().is_empty());
        });
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn remove_nonexistent_plugin_returns_error() {
        let tmp = unique_temp_dir("remove-missing");
        let home = tmp.join("home");
        with_home(&home, || {
            let mut manager = PluginManager::new(None);
            let error = manager.remove_installed("missing", None).unwrap_err();
            assert!(matches!(error, PluginLoadError::NotUserInstalled(_)));
        });
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn remove_does_not_delete_project_plugin() {
        let tmp = unique_temp_dir("remove-project");
        let home = tmp.join("home");
        let project = tmp.join("project");
        let project_plugin = project.join(".claude").join("plugins").join("team-plugin");
        write_plugin(&project_plugin, "team-plugin", "1.0.0");

        with_home(&home, || {
            let mut manager = PluginManager::new(Some(&project));
            let error = manager
                .remove_installed("team-plugin", Some(&project))
                .unwrap_err();
            assert!(matches!(error, PluginLoadError::NotUserInstalled(_)));
            assert!(project_plugin.exists());
        });

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
