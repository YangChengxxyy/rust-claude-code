//! Skills discovery entry point (iteration 50).
//!
//! Resolves the default skills directories — user (`~/.claude/skills`) and
//! project (`.claude/skills`) — and delegates the actual parsing to
//! [`rust_claude_core::skills::SkillRegistry`]. This mirrors the plugin
//! loader's precedence: **project skills override user skills** on name
//! collision, because the project directory is scanned second.
//!
//! Nothing here executes a skill; the result is a list of definitions for
//! slash-suggestion and the future `SkillTool`.

use std::path::{Path, PathBuf};

use rust_claude_core::skills::{Skill, SkillLoadError, SkillRegistry, SkillSource};

/// Discovers and holds skills loaded from the default user/project dirs.
pub struct SkillLoader {
    registry: SkillRegistry,
}

impl SkillLoader {
    /// Discover skills from `~/.claude/skills` and, if given, the project's
    /// `.claude/skills`. Project skills take precedence over user skills with
    /// the same name.
    pub fn discover(project_dir: Option<&Path>) -> Self {
        let mut dirs: Vec<(PathBuf, SkillSource)> = Vec::new();

        if let Some(user_dir) = user_skills_dir() {
            dirs.push((user_dir, SkillSource::User));
        }

        if let Some(project_dir) = project_dir {
            let project_skills_dir = project_dir.join(".claude").join("skills");
            if project_skills_dir.is_dir() {
                dirs.push((project_skills_dir, SkillSource::Project));
            }
        }

        Self {
            registry: SkillRegistry::load_from_dirs(&dirs),
        }
    }

    /// Build a loader from an existing registry (mainly for tests).
    pub fn from_registry(registry: SkillRegistry) -> Self {
        Self { registry }
    }

    /// Consume the loader and return the underlying registry. The CLI uses
    /// this to share one discovered registry between `SkillTool` and the TUI
    /// slash-suggestion registration.
    pub fn into_registry(self) -> SkillRegistry {
        self.registry
    }

    /// Loaded skills, sorted by name.
    pub fn skills(&self) -> Vec<&Skill> {
        self.registry.list()
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.registry.get(name)
    }

    /// Recoverable parse errors collected during discovery.
    pub fn errors(&self) -> &[SkillLoadError] {
        self.registry.errors()
    }

    /// Number of loaded skills.
    pub fn len(&self) -> usize {
        self.registry.len()
    }

    /// Whether any skills were loaded.
    pub fn is_empty(&self) -> bool {
        self.registry.is_empty()
    }
}

/// Resolve the user skills directory: `$HOME/.claude/skills`.
fn user_skills_dir() -> Option<PathBuf> {
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
    Some(Path::new(&home).join(".claude").join("skills"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Serializes tests that mutate `$HOME` so they don't race each other.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn with_home<T>(home: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = env_lock();
        let old_home = std::env::var("HOME").ok();
        // SAFETY: test-only env mutation, serialized by `env_lock`.
        unsafe { std::env::set_var("HOME", home) };
        let result = f();
        match old_home {
            Some(value) => unsafe { std::env::set_var("HOME", value) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        result
    }

    fn write_skill(dir: &Path, rel: &str, name: &str, description: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            format!("---\nname: {name}\ndescription: {description}\n---\nbody\n"),
        )
        .unwrap();
    }

    fn unique_tmp(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rust-claude-skill-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn discover_finds_user_skill() {
        let tmp = unique_tmp("user-only");
        let home = tmp.join("home");
        write_skill(&home.join(".claude").join("skills"), "greet.md", "greet", "Greets");

        with_home(&home, || {
            let loader = SkillLoader::discover(None);
            assert_eq!(loader.len(), 1);
            assert_eq!(loader.get("greet").unwrap().source, SkillSource::User);
        });

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn discover_combines_user_and_project_skills() {
        let tmp = unique_tmp("combine");
        let home = tmp.join("home");
        let project = tmp.join("project");
        write_skill(&home.join(".claude").join("skills"), "a.md", "alpha", "user alpha");
        write_skill(
            &project.join(".claude").join("skills"),
            "b.md",
            "beta",
            "project beta",
        );

        with_home(&home, || {
            let loader = SkillLoader::discover(Some(&project));
            assert_eq!(loader.len(), 2);
            assert!(loader.get("alpha").is_some());
            assert_eq!(loader.get("beta").unwrap().source, SkillSource::Project);
        });

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn project_overrides_user_skill_with_same_name() {
        let tmp = unique_tmp("override");
        let home = tmp.join("home");
        let project = tmp.join("project");
        write_skill(&home.join(".claude").join("skills"), "shared.md", "shared", "user");
        write_skill(
            &project.join(".claude").join("skills"),
            "shared.md",
            "shared",
            "project",
        );

        with_home(&home, || {
            let loader = SkillLoader::discover(Some(&project));
            assert_eq!(loader.len(), 1);
            let skill = loader.get("shared").unwrap();
            assert_eq!(skill.description, "project");
            assert_eq!(skill.source, SkillSource::Project);
        });

        let _ = std::fs::remove_dir_all(tmp);
    }

    #[test]
    fn discover_without_any_dirs_is_empty() {
        let tmp = unique_tmp("empty");
        let home = tmp.join("home");
        // HOME set but no skills dir; no project dir either.
        with_home(&home, || {
            let loader = SkillLoader::discover(None);
            assert!(loader.is_empty());
            assert!(loader.errors().is_empty());
        });
        let _ = std::fs::remove_dir_all(tmp);
    }
}
