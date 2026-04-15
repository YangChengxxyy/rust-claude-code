use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub is_read_only: bool,
    pub is_concurrency_safe: bool,
}

pub struct ToolRegistry {
    tools: HashMap<String, ToolInfo>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, info: ToolInfo) {
        self.tools.insert(info.name.clone(), info);
    }

    pub fn get(&self, name: &str) -> Option<&ToolInfo> {
        self.tools.get(name)
    }

    pub fn list(&self) -> Vec<&ToolInfo> {
        let mut tools: Vec<&ToolInfo> = self.tools.values().collect();
        tools.sort_by(|a, b| a.name.cmp(&b.name));
        tools
    }

    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.tools.keys().map(|s| s.as_str()).collect();
        names.sort();
        names
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolInfo {
            name: "Bash".to_string(),
            description: "Run bash".to_string(),
            input_schema: serde_json::json!({}),
            is_read_only: false,
            is_concurrency_safe: false,
        });

        assert!(registry.get("Bash").is_some());
        assert!(registry.get("Unknown").is_none());
    }

    #[test]
    fn test_list_sorted() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolInfo {
            name: "FileWrite".to_string(),
            description: "Write file".to_string(),
            input_schema: serde_json::json!({}),
            is_read_only: false,
            is_concurrency_safe: false,
        });
        registry.register(ToolInfo {
            name: "Bash".to_string(),
            description: "Run bash".to_string(),
            input_schema: serde_json::json!({}),
            is_read_only: false,
            is_concurrency_safe: false,
        });

        let names = registry.names();
        assert_eq!(names, vec!["Bash", "FileWrite"]);
    }
}
