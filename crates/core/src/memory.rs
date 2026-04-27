use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const MEMORY_ENTRYPOINT_NAME: &str = "MEMORY.md";
const MEMORY_ENTRY_MAX_LINES: usize = 200;
const MEMORY_ENTRY_MAX_BYTES: usize = 16_000;
const MEMORY_SCAN_LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    User,
    Feedback,
    Project,
    Reference,
}

impl MemoryType {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "user" => Some(Self::User),
            "feedback" => Some(Self::Feedback),
            "project" => Some(Self::Project),
            "reference" => Some(Self::Reference),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Feedback => "feedback",
            Self::Project => "project",
            Self::Reference => "reference",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryFrontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    pub memory_type: Option<MemoryType>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub path: PathBuf,
    pub relative_path: String,
    pub modified_at_ms: u64,
    pub freshness_days: u64,
    pub frontmatter: MemoryFrontmatter,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryIndex {
    pub path: PathBuf,
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryStore {
    pub project_root: PathBuf,
    pub memory_dir: PathBuf,
    pub entrypoint: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannedMemoryStore {
    pub store: MemoryStore,
    pub index: Option<MemoryIndex>,
    pub entries: Vec<MemoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelevantMemory {
    pub relative_path: String,
    pub memory_type: Option<MemoryType>,
    pub description: Option<String>,
    pub freshness_days: u64,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryWriteRequest {
    pub relative_path: String,
    pub frontmatter: MemoryFrontmatter,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoMemoryCandidate {
    pub request: MemoryWriteRequest,
    pub trigger: AutoMemoryTrigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoMemoryTrigger {
    UserCorrection,
    StablePreference,
    ProjectContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySaveOutcome {
    Created { path: PathBuf },
    Updated { path: PathBuf, previous_path: String },
    Skipped { reason: MemorySaveSkipReason },
}

impl MemorySaveOutcome {
    pub fn describe(&self) -> String {
        match self {
            Self::Created { path } => format!("created {}", path.display()),
            Self::Updated {
                path,
                previous_path,
            } => format!("updated {} at {}", previous_path, path.display()),
            Self::Skipped { reason } => format!("skipped: {}", reason.as_str()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySaveSkipReason {
    AutoMemoryDisabled,
    StoreUnavailable,
}

impl MemorySaveSkipReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AutoMemoryDisabled => "auto-memory disabled",
            Self::StoreUnavailable => "memory store unavailable",
        }
    }
}

pub fn is_auto_memory_disabled() -> bool {
    std::env::var("CLAUDE_CODE_DISABLE_AUTO_MEMORY")
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true"))
        .unwrap_or(false)
}

pub fn is_auto_memory_disabled_from_env<I, K, V>(vars: I) -> bool
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    vars.into_iter().any(|(key, value)| {
        key.as_ref() == "CLAUDE_CODE_DISABLE_AUTO_MEMORY"
            && matches!(
                value.as_ref().trim().to_ascii_lowercase().as_str(),
                "1" | "true"
            )
    })
}

pub fn build_memory_contract_prompt() -> String {
    build_memory_contract_prompt_with_auto_memory(!is_auto_memory_disabled())
}

pub fn build_memory_contract_prompt_with_auto_memory(auto_memory_enabled: bool) -> String {
    let auto_memory_section = if auto_memory_enabled {
        [
            "## Automatic memory",
            "- You may request an automatic memory save only for durable corrections, stable user preferences, or non-derivable project context that should persist across sessions.",
            "- Automatic memory is opportunistic and policy-gated; explicit user requests to remember or forget are authoritative.",
            "- Prefer updating an existing relevant memory over creating a duplicate.",
        ]
    } else {
        [
            "## Automatic memory",
            "- Automatic memory writes are disabled by environment configuration.",
            "- Do not request automatic memory saves, but continue to follow explicit user memory commands.",
            "- Manual memory inspection, remember, and forget behavior remains available.",
        ]
    };

    [
        "# memoryContract",
        "",
        "Use memory to preserve durable, non-derivable context across sessions.",
        "",
        "## Types of memory",
        "- `user`: stable information about the user's role, preferences, responsibilities, and knowledge.",
        "- `feedback`: guidance about how to approach work, including what to avoid and what to keep doing.",
        "- `project`: ongoing work, goals, incidents, or constraints that are not derivable from code or git history.",
        "- `reference`: pointers to external systems or resources where current information can be found.",
        "",
        "## What NOT to save in memory",
        "- Code patterns, architecture, file paths, or project structure that can be derived from the repository.",
        "- Git history, recent changes, or who changed what.",
        "- Debugging recipes whose authoritative source is the code or commit history.",
        "- Anything already documented in CLAUDE.md or equivalent instruction files.",
        "- Secrets, credentials, API keys, tokens, or private values.",
        "- Anything the user explicitly says not to remember.",
        "- Ephemeral task state or current conversation-only details.",
        "",
        auto_memory_section[0],
        auto_memory_section[1],
        auto_memory_section[2],
        auto_memory_section[3],
        "",
        "## When to access memory",
        "- When memory seems relevant to the user's request or prior-conversation work.",
        "- You MUST access memory when the user explicitly asks you to check, recall, or remember.",
        "- If the user says to ignore or not use memory, proceed as if memory were empty for that response.",
        "",
        "## Before trusting recalled memory",
        "- Memory is historical context, not live repository truth.",
        "- If a memory names a file, function, or flag, verify it against the current code before recommending it as current fact.",
        "- Prefer current code and git state over stale memory when they disagree.",
    ]
    .join("\n")
}

pub fn discover_memory_store(cwd: &Path) -> Option<MemoryStore> {
    let project_root = crate::claude_md::find_git_root(cwd).unwrap_or_else(|| cwd.to_path_buf());
    let home = std::env::var("HOME").ok()?;
    let sanitized = sanitize_project_root(&project_root);
    let memory_dir = PathBuf::from(home)
        .join(".claude")
        .join("projects")
        .join(sanitized)
        .join("memory");
    let entrypoint = memory_dir.join(MEMORY_ENTRYPOINT_NAME);
    Some(MemoryStore {
        project_root,
        memory_dir,
        entrypoint,
    })
}

pub fn scan_memory_store(store: &MemoryStore) -> std::io::Result<ScannedMemoryStore> {
    let index = load_memory_index(&store.entrypoint)?;
    let mut entries = Vec::new();

    if store.memory_dir.exists() {
        collect_memory_entries(&store.memory_dir, &store.memory_dir, &mut entries)?;
        entries.sort_by(|a, b| b.modified_at_ms.cmp(&a.modified_at_ms));
        entries.truncate(MEMORY_SCAN_LIMIT);
    }

    Ok(ScannedMemoryStore {
        store: store.clone(),
        index,
        entries,
    })
}

pub fn select_relevant_memories(
    scanned: &ScannedMemoryStore,
    query: &str,
    limit: usize,
) -> Vec<RelevantMemory> {
    let query_terms = tokenize(query);
    let mut scored = scanned
        .entries
        .iter()
        .filter_map(|entry| {
            let mut haystacks = vec![
                entry.relative_path.to_lowercase(),
                entry.body.to_lowercase(),
            ];
            if let Some(name) = &entry.frontmatter.name {
                haystacks.push(name.to_lowercase());
            }
            if let Some(description) = &entry.frontmatter.description {
                haystacks.push(description.to_lowercase());
            }

            let score = query_terms
                .iter()
                .filter(|term| {
                    haystacks
                        .iter()
                        .any(|haystack| haystack.contains(term.as_str()))
                })
                .count();

            if score == 0 {
                return None;
            }

            Some((
                score,
                entry.modified_at_ms,
                RelevantMemory {
                    relative_path: entry.relative_path.clone(),
                    memory_type: entry.frontmatter.memory_type,
                    description: entry.frontmatter.description.clone(),
                    freshness_days: entry.freshness_days,
                    body: entry.body.clone(),
                },
            ))
        })
        .collect::<Vec<_>>();

    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, memory)| memory)
        .collect()
}

pub fn build_relevant_memories_section(memories: &[RelevantMemory]) -> Option<String> {
    if memories.is_empty() {
        return None;
    }

    let mut lines = vec![
        "# relevantMemories".to_string(),
        String::new(),
        "Use these memories as historical context. Verify concrete file/function/flag claims against the current repository before treating them as present truth.".to_string(),
    ];

    for memory in memories {
        lines.push(String::new());
        let type_label = memory
            .memory_type
            .map(|t| t.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let description = memory.description.as_deref().unwrap_or("(no description)");
        lines.push(format!(
            "## [{}] {}\n- description: {}\n- freshness_days: {}\n\n{}",
            type_label, memory.relative_path, description, memory.freshness_days, memory.body
        ));
    }

    Some(lines.join("\n"))
}

/// Scan the memory store for an existing entry that would be a duplicate of
/// the given write request. Matches by exact `relative_path` first, then by
/// case-insensitive `frontmatter.name`.
pub fn find_duplicate_memory<'a>(
    scanned: &'a ScannedMemoryStore,
    request: &MemoryWriteRequest,
) -> Option<&'a MemoryEntry> {
    // 1. Exact path match
    if let Some(entry) = scanned
        .entries
        .iter()
        .find(|e| e.relative_path == request.relative_path)
    {
        return Some(entry);
    }

    // 2. Case-insensitive name match
    if let Some(req_name) = &request.frontmatter.name {
        let req_lower = req_name.to_ascii_lowercase();
        if let Some(entry) = scanned.entries.iter().find(|e| {
            e.frontmatter
                .name
                .as_ref()
                .map(|n| n.to_ascii_lowercase() == req_lower)
                .unwrap_or(false)
        }) {
            return Some(entry);
        }
    }

    None
}

pub fn write_memory_entry(
    store: &MemoryStore,
    request: &MemoryWriteRequest,
) -> std::io::Result<PathBuf> {
    let path = store.memory_dir.join(&request.relative_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = render_memory_file(&request.frontmatter, &request.body);
    fs::write(&path, content)?;
    rebuild_memory_index(store)?;
    Ok(path)
}

pub fn save_memory_entry_dedup(
    store: &MemoryStore,
    request: &MemoryWriteRequest,
) -> std::io::Result<MemorySaveOutcome> {
    let scanned = scan_memory_store(store)?;
    if let Some(duplicate) = find_duplicate_memory(&scanned, request) {
        let previous_path = duplicate.relative_path.clone();
        let corrected_request = MemoryWriteRequest {
            relative_path: previous_path.clone(),
            frontmatter: request.frontmatter.clone(),
            body: request.body.clone(),
        };
        let path = correct_memory_entry(store, &corrected_request)?;
        return Ok(MemorySaveOutcome::Updated {
            path,
            previous_path,
        });
    }

    let path = write_memory_entry(store, request)?;
    Ok(MemorySaveOutcome::Created { path })
}

pub fn save_auto_memory_candidate(
    store: Option<&MemoryStore>,
    candidate: &AutoMemoryCandidate,
) -> std::io::Result<MemorySaveOutcome> {
    if is_auto_memory_disabled() {
        return Ok(MemorySaveOutcome::Skipped {
            reason: MemorySaveSkipReason::AutoMemoryDisabled,
        });
    }

    let Some(store) = store else {
        return Ok(MemorySaveOutcome::Skipped {
            reason: MemorySaveSkipReason::StoreUnavailable,
        });
    };

    save_memory_entry_dedup(store, &candidate.request)
}

/// Update an existing memory file in-place with new frontmatter and body,
/// then rebuild the `MEMORY.md` index. Returns an error if the target file
/// does not exist (use `write_memory_entry` for creation).
pub fn correct_memory_entry(
    store: &MemoryStore,
    request: &MemoryWriteRequest,
) -> std::io::Result<PathBuf> {
    let path = store.memory_dir.join(&request.relative_path);
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Memory file not found: {}", request.relative_path),
        ));
    }
    let content = render_memory_file(&request.frontmatter, &request.body);
    fs::write(&path, content)?;
    rebuild_memory_index(store)?;
    Ok(path)
}

pub fn remove_memory_entry(store: &MemoryStore, relative_path: &str) -> std::io::Result<bool> {
    let path = store.memory_dir.join(relative_path);
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(path)?;
    rebuild_memory_index(store)?;
    Ok(true)
}

pub fn rebuild_memory_index(store: &MemoryStore) -> std::io::Result<()> {
    fs::create_dir_all(&store.memory_dir)?;
    let scanned = scan_memory_store(store)?;
    let mut lines = Vec::new();
    for entry in &scanned.entries {
        let title = entry
            .frontmatter
            .name
            .clone()
            .unwrap_or_else(|| entry.relative_path.clone());
        let hook = entry.frontmatter.description.clone().unwrap_or_else(|| {
            format!(
                "{} memory",
                entry
                    .frontmatter
                    .memory_type
                    .map(|t| t.as_str())
                    .unwrap_or("unknown")
            )
        });
        lines.push(format!("- [{}]({}) - {}", title, entry.relative_path, hook));
    }
    let content = if lines.is_empty() {
        String::new()
    } else {
        lines.join("\n") + "\n"
    };
    fs::write(&store.entrypoint, content)
}

fn collect_memory_entries(
    root: &Path,
    dir: &Path,
    entries: &mut Vec<MemoryEntry>,
) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_memory_entries(root, &path, entries)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        if path.file_name().and_then(|s| s.to_str()) == Some(MEMORY_ENTRYPOINT_NAME) {
            continue;
        }

        let content = fs::read_to_string(&path)?;
        let (frontmatter, body) = parse_frontmatter(&content);
        let metadata = fs::metadata(&path)?;
        let modified_at_ms = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        entries.push(MemoryEntry {
            path,
            relative_path,
            modified_at_ms,
            freshness_days: freshness_days(modified_at_ms),
            frontmatter,
            body,
        });
    }
    Ok(())
}

fn load_memory_index(path: &Path) -> std::io::Result<Option<MemoryIndex>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(path)?;
    let truncated = truncate_entrypoint(&raw);
    Ok(Some(MemoryIndex {
        path: path.to_path_buf(),
        content: truncated.0,
        truncated: truncated.1,
    }))
}

fn truncate_entrypoint(raw: &str) -> (String, bool) {
    let mut bytes = 0usize;
    let mut lines = Vec::new();
    let mut truncated = false;
    for (idx, line) in raw.lines().enumerate() {
        let line_bytes = line.len() + 1;
        if idx >= MEMORY_ENTRY_MAX_LINES || bytes + line_bytes > MEMORY_ENTRY_MAX_BYTES {
            truncated = true;
            break;
        }
        lines.push(line);
        bytes += line_bytes;
    }
    (lines.join("\n"), truncated)
}

fn parse_frontmatter(content: &str) -> (MemoryFrontmatter, String) {
    let lines: Vec<&str> = content.lines().collect();
    if lines.first().copied() != Some("---") {
        return (MemoryFrontmatter::default(), content.trim().to_string());
    }

    let Some(end_idx) =
        lines.iter().enumerate().skip(1).find_map(
            |(i, line)| {
                if *line == "---" {
                    Some(i)
                } else {
                    None
                }
            },
        )
    else {
        return (MemoryFrontmatter::default(), content.trim().to_string());
    };

    let mut frontmatter = MemoryFrontmatter::default();
    for line in &lines[1..end_idx] {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().to_string();
        match key {
            "name" => frontmatter.name = Some(value),
            "description" => frontmatter.description = Some(value),
            "type" => frontmatter.memory_type = MemoryType::parse(&value),
            _ => {
                frontmatter.extra.insert(key.to_string(), value);
            }
        }
    }

    let body = lines[end_idx + 1..].join("\n").trim().to_string();
    (frontmatter, body)
}

fn freshness_days(modified_at_ms: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(modified_at_ms);
    now.saturating_sub(modified_at_ms) / 86_400_000
}

fn sanitize_project_root(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn tokenize(input: &str) -> Vec<String> {
    input
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-')
        .filter(|term| term.len() >= 3)
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

fn render_memory_file(frontmatter: &MemoryFrontmatter, body: &str) -> String {
    let mut lines = vec!["---".to_string()];
    if let Some(name) = &frontmatter.name {
        lines.push(format!("name: {}", name));
    }
    if let Some(description) = &frontmatter.description {
        lines.push(format!("description: {}", description));
    }
    if let Some(memory_type) = frontmatter.memory_type {
        lines.push(format!("type: {}", memory_type.as_str()));
    }
    for (key, value) in &frontmatter.extra {
        lines.push(format!("{}: {}", key, value));
    }
    lines.push("---".to_string());
    lines.push(String::new());
    lines.push(body.trim().to_string());
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_memory_type() {
        assert_eq!(MemoryType::parse("user"), Some(MemoryType::User));
        assert_eq!(MemoryType::parse("feedback"), Some(MemoryType::Feedback));
        assert_eq!(MemoryType::parse("unknown"), None);
    }

    #[test]
    fn builds_memory_contract_prompt() {
        let prompt = build_memory_contract_prompt();
        assert!(prompt.contains("# memoryContract"));
        assert!(prompt.contains("What NOT to save"));
        assert!(prompt.contains("ignore or not use memory"));
    }

    #[test]
    fn builds_enabled_auto_memory_guidance() {
        let prompt = build_memory_contract_prompt_with_auto_memory(true);
        assert!(prompt.contains("Automatic memory"));
        assert!(prompt.contains("durable corrections"));
        assert!(prompt.contains("stable user preferences"));
        assert!(prompt.contains("Secrets, credentials"));
    }

    #[test]
    fn builds_disabled_auto_memory_guidance() {
        let prompt = build_memory_contract_prompt_with_auto_memory(false);
        assert!(prompt.contains("Automatic memory writes are disabled"));
        assert!(prompt.contains("Do not request automatic memory saves"));
        assert!(prompt.contains("explicit user memory commands"));
    }

    #[test]
    fn detects_auto_memory_disable_flag() {
        assert!(is_auto_memory_disabled_from_env([(
            "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
            "1"
        )]));
        assert!(is_auto_memory_disabled_from_env([(
            "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
            "true"
        )]));
        assert!(!is_auto_memory_disabled_from_env([(
            "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
            "0"
        )]));
        assert!(!is_auto_memory_disabled_from_env([("OTHER", "true")]));
    }

    #[test]
    fn parses_frontmatter_and_body() {
        let content = "---\nname: Test\ndescription: Desc\ntype: project\nfoo: bar\n---\n\nBody";
        let (frontmatter, body) = parse_frontmatter(content);
        assert_eq!(frontmatter.name.as_deref(), Some("Test"));
        assert_eq!(frontmatter.description.as_deref(), Some("Desc"));
        assert_eq!(frontmatter.memory_type, Some(MemoryType::Project));
        assert_eq!(
            frontmatter.extra.get("foo").map(String::as_str),
            Some("bar")
        );
        assert_eq!(body, "Body");
    }

    #[test]
    fn truncates_memory_entrypoint() {
        let raw = (0..210)
            .map(|i| format!("- line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (content, truncated) = truncate_entrypoint(&raw);
        assert!(truncated);
        assert!(content.lines().count() <= MEMORY_ENTRY_MAX_LINES);
    }

    #[test]
    fn selects_relevant_memories_by_query_terms() {
        let scanned = ScannedMemoryStore {
            store: MemoryStore {
                project_root: PathBuf::from("/repo"),
                memory_dir: PathBuf::from("/memory"),
                entrypoint: PathBuf::from("/memory/MEMORY.md"),
            },
            index: None,
            entries: vec![
                MemoryEntry {
                    path: PathBuf::from("/memory/testing.md"),
                    relative_path: "testing.md".to_string(),
                    modified_at_ms: 2,
                    freshness_days: 1,
                    frontmatter: MemoryFrontmatter {
                        description: Some("Use real database in tests".to_string()),
                        memory_type: Some(MemoryType::Feedback),
                        ..MemoryFrontmatter::default()
                    },
                    body: "Use real database integration tests".to_string(),
                },
                MemoryEntry {
                    path: PathBuf::from("/memory/ui.md"),
                    relative_path: "ui.md".to_string(),
                    modified_at_ms: 1,
                    freshness_days: 2,
                    frontmatter: MemoryFrontmatter {
                        description: Some("Frontend polish guidance".to_string()),
                        memory_type: Some(MemoryType::Project),
                        ..MemoryFrontmatter::default()
                    },
                    body: "Prefer stronger visual hierarchy in dashboard UI".to_string(),
                },
            ],
        };

        let selected = select_relevant_memories(&scanned, "fix database tests", 5);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].relative_path, "testing.md");
    }

    #[test]
    fn writes_memory_entry_and_rebuilds_index() {
        let dir = std::env::temp_dir().join(format!("memory-write-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = MemoryStore {
            project_root: PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };

        let request = MemoryWriteRequest {
            relative_path: "feedback/testing.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Testing".to_string()),
                description: Some("Use real DB in tests".to_string()),
                memory_type: Some(MemoryType::Feedback),
                ..MemoryFrontmatter::default()
            },
            body: "Use real database integration tests.".to_string(),
        };

        let path = write_memory_entry(&store, &request).unwrap();
        assert!(path.exists());
        let index = fs::read_to_string(store.entrypoint).unwrap();
        assert!(index.contains("[Testing](feedback/testing.md)"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn removes_memory_entry_and_updates_index() {
        let dir = std::env::temp_dir().join(format!("memory-remove-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("feedback")).unwrap();
        fs::write(
            dir.join("feedback/testing.md"),
            "---\nname: Testing\ndescription: Use real DB in tests\ntype: feedback\n---\n\nUse real DB.\n",
        )
        .unwrap();
        let store = MemoryStore {
            project_root: PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };
        rebuild_memory_index(&store).unwrap();

        let removed = remove_memory_entry(&store, "feedback/testing.md").unwrap();
        assert!(removed);
        let index = fs::read_to_string(store.entrypoint).unwrap();
        assert!(index.trim().is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_duplicate_by_exact_path() {
        let scanned = ScannedMemoryStore {
            store: MemoryStore {
                project_root: PathBuf::from("/repo"),
                memory_dir: PathBuf::from("/memory"),
                entrypoint: PathBuf::from("/memory/MEMORY.md"),
            },
            index: None,
            entries: vec![MemoryEntry {
                path: PathBuf::from("/memory/testing.md"),
                relative_path: "testing.md".to_string(),
                modified_at_ms: 1,
                freshness_days: 0,
                frontmatter: MemoryFrontmatter {
                    name: Some("Testing".to_string()),
                    ..MemoryFrontmatter::default()
                },
                body: "body".to_string(),
            }],
        };

        let request = MemoryWriteRequest {
            relative_path: "testing.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Different Name".to_string()),
                ..MemoryFrontmatter::default()
            },
            body: "new body".to_string(),
        };

        let dup = find_duplicate_memory(&scanned, &request);
        assert!(dup.is_some());
        assert_eq!(dup.unwrap().relative_path, "testing.md");
    }

    #[test]
    fn find_duplicate_by_name_case_insensitive() {
        let scanned = ScannedMemoryStore {
            store: MemoryStore {
                project_root: PathBuf::from("/repo"),
                memory_dir: PathBuf::from("/memory"),
                entrypoint: PathBuf::from("/memory/MEMORY.md"),
            },
            index: None,
            entries: vec![MemoryEntry {
                path: PathBuf::from("/memory/testing.md"),
                relative_path: "testing.md".to_string(),
                modified_at_ms: 1,
                freshness_days: 0,
                frontmatter: MemoryFrontmatter {
                    name: Some("Testing Conventions".to_string()),
                    ..MemoryFrontmatter::default()
                },
                body: "body".to_string(),
            }],
        };

        let request = MemoryWriteRequest {
            relative_path: "other.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("testing conventions".to_string()),
                ..MemoryFrontmatter::default()
            },
            body: "new body".to_string(),
        };

        let dup = find_duplicate_memory(&scanned, &request);
        assert!(dup.is_some());
        assert_eq!(dup.unwrap().relative_path, "testing.md");
    }

    #[test]
    fn find_duplicate_returns_none_when_no_match() {
        let scanned = ScannedMemoryStore {
            store: MemoryStore {
                project_root: PathBuf::from("/repo"),
                memory_dir: PathBuf::from("/memory"),
                entrypoint: PathBuf::from("/memory/MEMORY.md"),
            },
            index: None,
            entries: vec![MemoryEntry {
                path: PathBuf::from("/memory/testing.md"),
                relative_path: "testing.md".to_string(),
                modified_at_ms: 1,
                freshness_days: 0,
                frontmatter: MemoryFrontmatter {
                    name: Some("Testing".to_string()),
                    ..MemoryFrontmatter::default()
                },
                body: "body".to_string(),
            }],
        };

        let request = MemoryWriteRequest {
            relative_path: "other.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Deployment".to_string()),
                ..MemoryFrontmatter::default()
            },
            body: "new body".to_string(),
        };

        assert!(find_duplicate_memory(&scanned, &request).is_none());
    }

    #[test]
    fn correct_memory_entry_updates_existing_file() {
        let dir = std::env::temp_dir().join(format!("memory-correct-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = MemoryStore {
            project_root: PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };

        // Create an initial entry
        let initial = MemoryWriteRequest {
            relative_path: "testing.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Testing".to_string()),
                description: Some("Old description".to_string()),
                memory_type: Some(MemoryType::Feedback),
                ..MemoryFrontmatter::default()
            },
            body: "Old body.".to_string(),
        };
        write_memory_entry(&store, &initial).unwrap();

        // Correct it
        let correction = MemoryWriteRequest {
            relative_path: "testing.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Testing".to_string()),
                description: Some("Updated description".to_string()),
                memory_type: Some(MemoryType::Feedback),
                ..MemoryFrontmatter::default()
            },
            body: "Updated body.".to_string(),
        };
        let path = correct_memory_entry(&store, &correction).unwrap();
        assert!(path.exists());

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Updated description"));
        assert!(content.contains("Updated body."));
        assert!(!content.contains("Old"));

        let index = fs::read_to_string(&store.entrypoint).unwrap();
        assert!(index.contains("[Testing](testing.md)"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn correct_memory_entry_errors_on_nonexistent_target() {
        let dir =
            std::env::temp_dir().join(format!("memory-correct-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = MemoryStore {
            project_root: PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };

        let request = MemoryWriteRequest {
            relative_path: "nonexistent.md".to_string(),
            frontmatter: MemoryFrontmatter::default(),
            body: "body".to_string(),
        };

        let result = correct_memory_entry(&store, &request);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_memory_entry_dedup_creates_new_entry() {
        let dir = std::env::temp_dir().join(format!("memory-dedup-new-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = MemoryStore {
            project_root: PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };

        let request = MemoryWriteRequest {
            relative_path: "project/context.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Project Context".to_string()),
                memory_type: Some(MemoryType::Project),
                ..MemoryFrontmatter::default()
            },
            body: "Stable project context.".to_string(),
        };

        let outcome = save_memory_entry_dedup(&store, &request).unwrap();
        assert!(matches!(outcome, MemorySaveOutcome::Created { .. }));
        assert!(dir.join("project/context.md").exists());
        assert!(fs::read_to_string(&store.entrypoint)
            .unwrap()
            .contains("[Project Context](project/context.md)"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_memory_entry_dedup_updates_duplicate_path() {
        let dir = std::env::temp_dir().join(format!("memory-dedup-path-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = MemoryStore {
            project_root: PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };
        let initial = MemoryWriteRequest {
            relative_path: "feedback/testing.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Testing".to_string()),
                memory_type: Some(MemoryType::Feedback),
                ..MemoryFrontmatter::default()
            },
            body: "Old body.".to_string(),
        };
        write_memory_entry(&store, &initial).unwrap();

        let update = MemoryWriteRequest {
            relative_path: "feedback/testing.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Testing".to_string()),
                memory_type: Some(MemoryType::Feedback),
                ..MemoryFrontmatter::default()
            },
            body: "New body.".to_string(),
        };

        let outcome = save_memory_entry_dedup(&store, &update).unwrap();
        assert!(matches!(outcome, MemorySaveOutcome::Updated { .. }));
        let content = fs::read_to_string(dir.join("feedback/testing.md")).unwrap();
        assert!(content.contains("New body."));
        assert!(!content.contains("Old body."));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_memory_entry_dedup_updates_duplicate_name_at_existing_path() {
        let dir = std::env::temp_dir().join(format!("memory-dedup-name-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = MemoryStore {
            project_root: PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };
        let initial = MemoryWriteRequest {
            relative_path: "feedback/testing.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Testing".to_string()),
                memory_type: Some(MemoryType::Feedback),
                ..MemoryFrontmatter::default()
            },
            body: "Old body.".to_string(),
        };
        write_memory_entry(&store, &initial).unwrap();

        let update = MemoryWriteRequest {
            relative_path: "feedback/other.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("testing".to_string()),
                memory_type: Some(MemoryType::Feedback),
                ..MemoryFrontmatter::default()
            },
            body: "New body.".to_string(),
        };

        let outcome = save_memory_entry_dedup(&store, &update).unwrap();
        assert!(matches!(
            outcome,
            MemorySaveOutcome::Updated {
                previous_path,
                ..
            } if previous_path == "feedback/testing.md"
        ));
        assert!(dir.join("feedback/testing.md").exists());
        assert!(!dir.join("feedback/other.md").exists());
        assert!(fs::read_to_string(dir.join("feedback/testing.md"))
            .unwrap()
            .contains("New body."));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn manual_memory_save_ignores_auto_memory_disable_policy() {
        let dir = std::env::temp_dir().join(format!("memory-manual-save-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let store = MemoryStore {
            project_root: PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };
        let request = MemoryWriteRequest {
            relative_path: "user/style.md".to_string(),
            frontmatter: MemoryFrontmatter {
                name: Some("Style".to_string()),
                memory_type: Some(MemoryType::User),
                ..MemoryFrontmatter::default()
            },
            body: "Prefer concise answers.".to_string(),
        };

        assert!(is_auto_memory_disabled_from_env([(
            "CLAUDE_CODE_DISABLE_AUTO_MEMORY",
            "1"
        )]));
        let outcome = save_memory_entry_dedup(&store, &request).unwrap();
        assert!(matches!(outcome, MemorySaveOutcome::Created { .. }));

        let _ = fs::remove_dir_all(&dir);
    }
}
