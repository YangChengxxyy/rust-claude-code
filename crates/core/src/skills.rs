//! Local Skills discovery + Markdown frontmatter parsing (iteration 50).
//!
//! Minimal, **non-executing** skills loader. It discovers Markdown skill files
//! from one or more directories, parses their YAML frontmatter (`name`,
//! `description`, `allowed-tools`, `trigger`), and returns a registry of
//! [`Skill`] definitions that slash-suggestion and the future `SkillTool`
//! (iteration 51) can consume. No skill is *executed* here.
//!
//! Discovery is split across crates on purpose:
//! - This module (core) owns the data model, frontmatter parsing, and a
//!   directory-list-driven loader (`SkillRegistry::load_from_dirs`) that is
//!   fully testable with temp dirs (no `$HOME`).
//! - The SDK (`rust_claude_sdk::skill`) resolves the default user/project
//!   directories and calls into this loader, mirroring the plugin loader.
//!
//! Override rule: directories are processed in the order given, and a skill
//! parsed from a later directory replaces any earlier skill with the same
//! `name`. Callers pass user dirs first and the project dir last, so **project
//! skills override user skills** on name collision (matching plugin/agent
//! precedence). A parse failure for one file is collected as an error and does
//! NOT abort the rest of the scan.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Where a skill was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSource {
    /// User-wide skills directory (`~/.claude/skills`).
    User,
    /// Project-local skills directory (`.claude/skills`).
    Project,
}

/// A parsed skill definition loaded from a local Markdown file.
///
/// `body` is the Markdown content after the frontmatter; iteration 51's
/// `SkillTool` will surface it as the skill prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    /// Tools the skill is allowed to use, from the `allowed-tools` field.
    pub allowed_tools: Vec<String>,
    /// Optional activation trigger from the `trigger` field.
    pub trigger: Option<String>,
    pub source: SkillSource,
    /// The file this skill was parsed from.
    pub path: PathBuf,
    /// Markdown body after the frontmatter.
    pub body: String,
}

/// A recoverable error encountered while parsing one skill file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillLoadError {
    pub path: PathBuf,
    pub message: String,
}

/// Registry of skills loaded from disk, keyed by skill `name`.
///
/// Later-loaded skills with a duplicate name replace earlier ones (see the
/// module-level override rule). All recoverable parse errors are retained in
/// [`SkillRegistry::errors`].
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: BTreeMap<String, Skill>,
    errors: Vec<SkillLoadError>,
}

impl SkillRegistry {
    /// Empty registry with no skills and no errors.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Load skills from `dirs` (in order). Each entry pairs a skills directory
    /// with the [`SkillSource`] to tag loaded skills with.
    pub fn load_from_dirs(dirs: &[(PathBuf, SkillSource)]) -> Self {
        let mut registry = Self::empty();
        for (dir, source) in dirs {
            for file_path in discover_skill_files(dir) {
                match parse_skill_file(&file_path, *source) {
                    Ok(skill) => {
                        registry.skills.insert(skill.name.clone(), skill);
                    }
                    Err(error) => registry.errors.push(error),
                }
            }
        }
        registry
    }

    /// All loaded skills, sorted by name.
    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// Recoverable parse errors collected during loading.
    pub fn errors(&self) -> &[SkillLoadError] {
        &self.errors
    }

    /// Number of successfully loaded skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Whether any skills were loaded.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

/// Enumerate candidate skill files under `dir`, sorted for deterministic
/// loading. A candidate is either:
/// - a subdirectory containing `SKILL.md`, or
/// - a top-level `*.md` file.
fn discover_skill_files(dir: &Path) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.is_file() {
                files.push(skill_md);
            }
        } else if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    files.sort();
    files
}

/// Parse a single skill file into a [`Skill`].
fn parse_skill_file(path: &Path, source: SkillSource) -> Result<Skill, SkillLoadError> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| load_error(path, &format!("failed to read skill: {error}")))?;
    parse_skill_definition(path, &content, source)
}

fn parse_skill_definition(
    path: &Path,
    content: &str,
    source: SkillSource,
) -> Result<Skill, SkillLoadError> {
    let rest = content
        .strip_prefix("---\n")
        .or_else(|| content.strip_prefix("---\r\n"))
        .ok_or_else(|| load_error(path, "missing YAML front matter"))?;

    // Find the closing `---` delimiter on its own line.
    let (frontmatter, body) = split_frontmatter(path, rest)?;
    let parsed: SkillFrontmatter = serde_yaml::from_str(frontmatter)
        .map_err(|error| load_error(path, &format!("invalid YAML front matter: {error}")))?;

    let name = required_field(path, "name", parsed.name)?;
    let description = required_field(path, "description", parsed.description)?;
    let allowed_tools = parsed
        .allowed_tools
        .map(normalize_tools)
        .unwrap_or_default();
    let body = body.trim().to_string();

    Ok(Skill {
        name,
        description,
        allowed_tools,
        trigger: parsed.trigger.map(|t| t.trim().to_string()),
        source,
        path: path.to_path_buf(),
        body,
    })
}

/// Split `rest` at the closing `---` delimiter line, returning the frontmatter
/// block and the body that follows it. The delimiter must be a line whose
/// trimmed content is exactly `---`, so values containing `---` are not
/// mistaken for the close.
fn split_frontmatter<'a>(path: &Path, rest: &'a str) -> Result<(&'a str, &'a str), SkillLoadError> {
    let bytes = rest.as_bytes();
    let mut search = 0;
    while let Some(rel) = rest[search..].find("---") {
        let abs = search + rel;
        let at_line_start = abs == 0 || bytes.get(abs - 1) == Some(&b'\n');
        if !at_line_start {
            search = abs + 3;
            continue;
        }
        let line_end = rest[abs..]
            .find('\n')
            .map(|i| abs + i)
            .unwrap_or(rest.len());
        if rest[abs..line_end].trim_end() != "---" {
            search = abs + 3;
            continue;
        }
        let frontmatter = &rest[..abs];
        let body_start = if line_end < rest.len() {
            line_end + 1
        } else {
            rest.len()
        };
        return Ok((frontmatter, &rest[body_start..]));
    }
    Err(load_error(path, "unterminated YAML front matter"))
}

/// `allowed-tools` may be a YAML list, a single tool, or a comma-separated
/// string — normalize any of these to a `Vec<String>`.
fn normalize_tools(field: ToolsField) -> Vec<String> {
    match field {
        ToolsField::List(items) => items
            .into_iter()
            .flat_map(|item| split_csv(&item))
            .collect(),
        ToolsField::Single(s) => split_csv(&s),
    }
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn required_field(
    path: &Path,
    field: &str,
    value: Option<String>,
) -> Result<String, SkillLoadError> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| load_error(path, &format!("missing required field '{field}'")))
}

fn load_error(path: &Path, message: &str) -> SkillLoadError {
    SkillLoadError {
        path: path.to_path_buf(),
        message: message.to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(default, alias = "allowed-tools")]
    allowed_tools: Option<ToolsField>,
    #[serde(default)]
    trigger: Option<String>,
}

/// `allowed-tools` accepts either a list or a (possibly comma-separated) scalar.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ToolsField {
    List(Vec<String>),
    Single(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill_md(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\nBody text.\n")
    }

    // ---- frontmatter parsing ----

    #[test]
    fn parses_minimal_skill() {
        let path = PathBuf::from("hello.md");
        let skill = parse_skill_definition(&path, &skill_md("hello", "Greets"), SkillSource::User)
            .unwrap();
        assert_eq!(skill.name, "hello");
        assert_eq!(skill.description, "Greets");
        assert!(skill.allowed_tools.is_empty());
        assert!(skill.trigger.is_none());
        assert_eq!(skill.source, SkillSource::User);
        assert_eq!(skill.body, "Body text.");
    }

    #[test]
    fn parses_allowed_tools_as_list_and_scalar_and_csv() {
        let path = PathBuf::from("s.md");

        // YAML block list.
        let list = "---\nname: s\ndescription: d\nallowed-tools:\n  - Bash\n  - FileRead\n---\nbody\n";
        let s = parse_skill_definition(&path, list, SkillSource::Project).unwrap();
        assert_eq!(s.allowed_tools, vec!["Bash", "FileRead"]);
        assert_eq!(s.source, SkillSource::Project);

        // Single scalar.
        let single = "---\nname: s\ndescription: d\nallowed-tools: Bash\n---\nbody\n";
        let s = parse_skill_definition(&path, single, SkillSource::User).unwrap();
        assert_eq!(s.allowed_tools, vec!["Bash"]);

        // Comma-separated scalar.
        let csv = "---\nname: s\ndescription: d\nallowed-tools: Bash, FileEdit, Glob\n---\nbody\n";
        let s = parse_skill_definition(&path, csv, SkillSource::User).unwrap();
        assert_eq!(s.allowed_tools, vec!["Bash", "FileEdit", "Glob"]);

        // Inline YAML list.
        let inline = "---\nname: s\ndescription: d\nallowed-tools: [Bash, FileWrite]\n---\nbody\n";
        let s = parse_skill_definition(&path, inline, SkillSource::User).unwrap();
        assert_eq!(s.allowed_tools, vec!["Bash", "FileWrite"]);
    }

    #[test]
    fn parses_trigger_field() {
        let path = PathBuf::from("s.md");
        let content =
            "---\nname: s\ndescription: d\ntrigger: on-commit\n---\nbody\n";
        let s = parse_skill_definition(&path, content, SkillSource::User).unwrap();
        assert_eq!(s.trigger.as_deref(), Some("on-commit"));
    }

    #[test]
    fn rejects_missing_required_fields() {
        let path = PathBuf::from("bad.md");
        let err = parse_skill_definition(&path, "---\nname: only\n---\nbody\n", SkillSource::User)
            .unwrap_err();
        assert!(err.message.contains("description"));
    }

    #[test]
    fn rejects_missing_frontmatter() {
        let path = PathBuf::from("bad.md");
        let err = parse_skill_definition(&path, "no frontmatter here", SkillSource::User).unwrap_err();
        assert!(err.message.contains("front matter"));
    }

    #[test]
    fn rejects_unterminated_frontmatter() {
        let path = PathBuf::from("bad.md");
        let err = parse_skill_definition(
            &path,
            "---\nname: s\ndescription: d\nbody without close",
            SkillSource::User,
        )
        .unwrap_err();
        assert!(err.message.contains("unterminated"));
    }

    // ---- directory loading + override ----

    fn write_skill(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn temp_dir(label: &str) -> PathBuf {
        let unique = format!(
            "skills-test-{}-{}-{}",
            label,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn loads_flat_file_and_directory_skill() {
        let dir = temp_dir("both");
        // Flat file.
        write_skill(&dir, "flat.md", &skill_md("flat", "A flat skill"));
        // Directory form with SKILL.md.
        write_skill(&dir, "nested/SKILL.md", &skill_md("nested", "A nested skill"));

        let registry = SkillRegistry::load_from_dirs(&[(dir.clone(), SkillSource::User)]);
        assert_eq!(registry.len(), 2);
        assert!(registry.get("flat").is_some());
        assert!(registry.get("nested").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn project_overrides_user_on_name_collision() {
        let user = temp_dir("user");
        let project = temp_dir("project");
        write_skill(&user, "shared.md", &skill_md("shared", "from user"));
        write_skill(&project, "shared.md", &skill_md("shared", "from project"));

        // User first, project last → project wins.
        let registry = SkillRegistry::load_from_dirs(&[
            (user.clone(), SkillSource::User),
            (project.clone(), SkillSource::Project),
        ]);
        assert_eq!(registry.len(), 1);
        let skill = registry.get("shared").unwrap();
        assert_eq!(skill.description, "from project");
        assert_eq!(skill.source, SkillSource::Project);

        let _ = std::fs::remove_dir_all(user);
        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn parse_error_does_not_abort_scan() {
        let dir = temp_dir("mixed");
        // One valid, one broken (missing frontmatter).
        write_skill(&dir, "good.md", &skill_md("good", "ok"));
        write_skill(&dir, "broken.md", "no frontmatter at all");

        let registry = SkillRegistry::load_from_dirs(&[(dir.clone(), SkillSource::User)]);
        assert_eq!(registry.len(), 1);
        assert!(registry.get("good").is_some());
        assert_eq!(registry.errors().len(), 1);
        assert!(registry.errors()[0].path.ends_with("broken.md"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missing_directory_yields_empty_registry() {
        let registry =
            SkillRegistry::load_from_dirs(&[(PathBuf::from("/no/such/skills"), SkillSource::User)]);
        assert!(registry.is_empty());
        assert!(registry.errors().is_empty());
    }

    #[test]
    fn list_is_sorted_by_name() {
        let dir = temp_dir("sorted");
        write_skill(&dir, "zeta.md", &skill_md("zeta", "z"));
        write_skill(&dir, "alpha.md", &skill_md("alpha", "a"));
        let registry = SkillRegistry::load_from_dirs(&[(dir.clone(), SkillSource::User)]);
        let names: Vec<&str> = registry.list().into_iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
        let _ = std::fs::remove_dir_all(dir);
    }
}
