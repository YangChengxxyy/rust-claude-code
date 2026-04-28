use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomAgentDefinition {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub model: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomAgentRegistry {
    agents: BTreeMap<String, CustomAgentDefinition>,
    errors: Vec<CustomAgentLoadError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomAgentLoadError {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Deserialize)]
struct AgentFrontmatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    model: Option<String>,
}

impl CustomAgentRegistry {
    pub fn discover(project_dir: &Path) -> Self {
        let agents_dir = project_dir.join(".claude").join("agents");
        if !agents_dir.is_dir() {
            return Self::empty();
        }

        let mut paths = match std::fs::read_dir(&agents_dir) {
            Ok(entries) => entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
                .collect::<Vec<_>>(),
            Err(error) => {
                return Self {
                    agents: BTreeMap::new(),
                    errors: vec![CustomAgentLoadError {
                        path: agents_dir,
                        message: format!("failed to read agents directory: {error}"),
                    }],
                };
            }
        };
        paths.sort();

        let mut registry = Self::empty();
        for path in paths {
            match parse_agent_file(&path) {
                Ok(agent) => {
                    if registry.agents.contains_key(&agent.name) {
                        registry.errors.push(CustomAgentLoadError {
                            path: agent.path.clone(),
                            message: format!("duplicate custom agent name '{}'", agent.name),
                        });
                    } else {
                        registry.agents.insert(agent.name.clone(), agent);
                    }
                }
                Err(error) => registry.errors.push(error),
            }
        }
        registry
    }

    pub fn empty() -> Self {
        Self {
            agents: BTreeMap::new(),
            errors: Vec::new(),
        }
    }

    pub fn from_agents(agents: Vec<CustomAgentDefinition>) -> Self {
        let mut registry = Self::empty();
        for agent in agents {
            registry.agents.insert(agent.name.clone(), agent);
        }
        registry
    }

    pub fn list(&self) -> Vec<&CustomAgentDefinition> {
        self.agents.values().collect()
    }

    pub fn get(&self, name: &str) -> Option<&CustomAgentDefinition> {
        self.agents.get(name)
    }

    pub fn errors(&self) -> &[CustomAgentLoadError] {
        &self.errors
    }

    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

fn parse_agent_file(path: &Path) -> Result<CustomAgentDefinition, CustomAgentLoadError> {
    let content = std::fs::read_to_string(path).map_err(|error| CustomAgentLoadError {
        path: path.to_path_buf(),
        message: format!("failed to read custom agent: {error}"),
    })?;
    parse_agent_definition(path, &content)
}

fn parse_agent_definition(
    path: &Path,
    content: &str,
) -> Result<CustomAgentDefinition, CustomAgentLoadError> {
    let Some(rest) = content.strip_prefix("---\n") else {
        return Err(load_error(path, "missing YAML front matter"));
    };
    let Some((frontmatter, body)) = rest.split_once("\n---\n") else {
        return Err(load_error(path, "unterminated YAML front matter"));
    };
    let frontmatter: AgentFrontmatter = serde_yaml::from_str(frontmatter)
        .map_err(|error| load_error(path, &format!("invalid YAML front matter: {error}")))?;

    let name = required_field(path, "name", frontmatter.name)?;
    if !is_kebab_case(&name) {
        return Err(load_error(path, "custom agent name must be kebab-case"));
    }
    let description = required_field(path, "description", frontmatter.description)?;
    let system_prompt = body.trim().to_string();
    if system_prompt.is_empty() {
        return Err(load_error(path, "system prompt body is empty"));
    }

    Ok(CustomAgentDefinition {
        name,
        description,
        system_prompt,
        tools: frontmatter.tools,
        model: frontmatter.model,
        path: path.to_path_buf(),
    })
}

fn required_field(
    path: &Path,
    field: &str,
    value: Option<String>,
) -> Result<String, CustomAgentLoadError> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| load_error(path, &format!("missing required field '{field}'")))
}

fn load_error(path: &Path, message: &str) -> CustomAgentLoadError {
    CustomAgentLoadError {
        path: path.to_path_buf(),
        message: message.to_string(),
    }
}

fn is_kebab_case(name: &str) -> bool {
    if name.is_empty() || name.starts_with('-') || name.ends_with('-') {
        return false;
    }

    name.split('-').all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("rust-claude-{name}-{nanos}"))
    }

    #[test]
    fn parses_valid_agent_definition() {
        let path = PathBuf::from("reviewer.md");
        let agent = parse_agent_definition(
            &path,
            "---\nname: reviewer\ndescription: Reviews code\ntools: [FileRead, Bash]\nmodel: claude-3-5-sonnet-latest\n---\nYou review code carefully.\n",
        )
        .unwrap();

        assert_eq!(agent.name, "reviewer");
        assert_eq!(agent.description, "Reviews code");
        assert_eq!(agent.tools, vec!["FileRead", "Bash"]);
        assert_eq!(agent.model.as_deref(), Some("claude-3-5-sonnet-latest"));
        assert_eq!(agent.system_prompt, "You review code carefully.");
    }

    #[test]
    fn rejects_missing_required_field() {
        let err = parse_agent_definition(Path::new("bad.md"), "---\nname: reviewer\n---\nPrompt\n")
            .unwrap_err();
        assert!(err.message.contains("description"));
    }

    #[test]
    fn rejects_invalid_name() {
        let err = parse_agent_definition(
            Path::new("bad.md"),
            "---\nname: Code Reviewer!\ndescription: Bad\n---\nPrompt\n",
        )
        .unwrap_err();
        assert!(err.message.contains("kebab-case"));
    }

    #[test]
    fn discovers_agents_and_reports_duplicates() {
        let root = temp_dir("agents");
        let agents_dir = root.join(".claude").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(
            agents_dir.join("a.md"),
            "---\nname: reviewer\ndescription: First\n---\nPrompt\n",
        )
        .unwrap();
        std::fs::write(
            agents_dir.join("b.md"),
            "---\nname: reviewer\ndescription: Second\n---\nPrompt\n",
        )
        .unwrap();

        let registry = CustomAgentRegistry::discover(&root);
        assert_eq!(registry.list().len(), 1);
        assert_eq!(registry.get("reviewer").unwrap().description, "First");
        assert_eq!(registry.errors().len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn missing_agents_directory_is_empty() {
        let registry = CustomAgentRegistry::discover(Path::new("/definitely/missing/project"));
        assert!(registry.is_empty());
        assert!(registry.errors().is_empty());
    }
}
