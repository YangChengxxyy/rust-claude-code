use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use rust_claude_core::tool_types::ToolInfo;

use crate::tool::{InterruptBehavior, Tool, ToolContext, ToolError};

#[derive(Clone)]
pub struct RegisteredTool {
    pub info: ToolInfo,
    pub is_read_only: bool,
    pub is_concurrency_safe: bool,
    pub should_defer: bool,
    pub interrupt_behavior: InterruptBehavior,
    pub tool: Arc<dyn Tool>,
}

pub struct ToolRegistry {
    tools: RwLock<HashMap<String, RegisteredTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            tools: RwLock::new(HashMap::new()),
        }
    }

    pub fn register<T>(&self, tool: T)
    where
        T: Tool + 'static,
    {
        self.register_arc(Arc::new(tool));
    }

    pub fn register_arc(&self, tool: Arc<dyn Tool>) {
        let info = tool.info();
        let mut map = self.tools.write().unwrap();
        map.insert(
            info.name.clone(),
            RegisteredTool {
                is_read_only: tool.is_read_only(),
                is_concurrency_safe: tool.is_concurrency_safe(),
                should_defer: tool.should_defer(),
                interrupt_behavior: tool.interrupt_behavior(),
                info,
                tool,
            },
        );
    }

    pub fn contains(&self, name: &str) -> bool {
        self.tools.read().unwrap().contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<RegisteredTool> {
        let map = self.tools.read().unwrap();
        map.get(name).cloned()
    }

    pub fn list(&self) -> Vec<RegisteredTool> {
        let map = self.tools.read().unwrap();
        let mut tools: Vec<RegisteredTool> = map.values().cloned().collect();
        tools.sort_by(|a, b| a.info.name.cmp(&b.info.name));
        tools
    }

    pub async fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        context: ToolContext,
    ) -> Result<rust_claude_core::tool_types::ToolResult, ToolError> {
        let tool = {
            let map = self.tools.read().unwrap();
            map.get(name)
                .cloned()
                .ok_or_else(|| ToolError::Execution(format!("unknown tool: {name}")))?
        };

        tool.tool.execute(input, context).await
    }

    pub fn names(&self) -> Vec<String> {
        let map = self.tools.read().unwrap();
        let mut names: Vec<String> = map.keys().cloned().collect();
        names.sort();
        names
    }

    pub fn is_concurrency_safe(&self, name: &str) -> bool {
        let map = self.tools.read().unwrap();
        map.get(name)
            .map(|tool| tool.is_concurrency_safe)
            .unwrap_or(false)
    }

    pub fn get_deferred_tools(&self) -> Vec<RegisteredTool> {
        let map = self.tools.read().unwrap();
        let mut tools: Vec<RegisteredTool> = map
            .values()
            .filter(|tool| tool.should_defer)
            .cloned()
            .collect();
        tools.sort_by(|a, b| a.info.name.cmp(&b.info.name));
        tools
    }

    pub fn get_non_deferred_tools(&self) -> Vec<RegisteredTool> {
        let map = self.tools.read().unwrap();
        let mut tools: Vec<RegisteredTool> = map
            .values()
            .filter(|tool| !tool.should_defer)
            .cloned()
            .collect();
        tools.sort_by(|a, b| a.info.name.cmp(&b.info.name));
        tools
    }

    pub fn search_tools(&self, query: &str, max: usize) -> Vec<RegisteredTool> {
        // Exact selection mode: select:ToolName
        if let Some(name) = query.strip_prefix("select:") {
            let name_lower = name.to_lowercase();
            let map = self.tools.read().unwrap();
            return map
                .values()
                .filter(|tool| tool.should_defer)
                .filter(|tool| tool.info.name.to_lowercase() == name_lower)
                .take(max)
                .cloned()
                .collect();
        }

        let query_lower = query.to_lowercase();
        let keywords: Vec<&str> = query_lower.split_whitespace().collect();

        let map = self.tools.read().unwrap();
        let mut scored: Vec<(i32, RegisteredTool)> = map
            .values()
            .filter(|tool| tool.should_defer)
            .filter_map(|tool| {
                let name_lower = tool.info.name.to_lowercase();
                let desc_lower = tool.info.description.to_lowercase();

                let name_tokens = tokenize_tool_name(&tool.info.name);
                let name_tokens_lower: Vec<String> =
                    name_tokens.iter().map(|s| s.to_lowercase()).collect();

                let mut score = 0i32;
                for kw in &keywords {
                    if name_lower.contains(kw) {
                        score += 2;
                    } else if desc_lower.contains(kw) {
                        score += 1;
                    } else {
                        // Check tokenized name parts
                        for token in &name_tokens_lower {
                            if token == kw {
                                score += 2;
                                break;
                            }
                        }
                    }
                }

                if score > 0 {
                    Some((score, tool.clone()))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.0.cmp(&a.0) // higher score first
                .then_with(|| a.1.info.name.cmp(&b.1.info.name)) // alphabetical tiebreak
        });

        scored.into_iter().map(|(_, tool)| tool).take(max).collect()
    }

    pub fn estimate_deferred_schema_tokens(&self) -> usize {
        let map = self.tools.read().unwrap();
        let total_chars: usize = map
            .values()
            .filter(|tool| tool.should_defer)
            .map(|tool| {
                let schema_str = serde_json::to_string(&tool.info.input_schema).unwrap_or_default();
                schema_str.len()
            })
            .sum();
        total_chars / 4
    }

    /// Filter tools: keep only those in the allow list (if non-empty),
    /// then remove any in the deny list.
    pub fn apply_tool_filters(&self, allowed: &[String], disallowed: &[String]) {
        let mut map = self.tools.write().unwrap();
        if !allowed.is_empty() {
            map.retain(|name, _| allowed.iter().any(|a| a == name));
        }
        for name in disallowed {
            map.remove(name);
        }
    }
}

fn tokenize_tool_name(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();

    // Split on __ first (MCP separator)
    for part in name.split("__") {
        // Then split on CamelCase boundaries
        let mut current = String::new();
        for ch in part.chars() {
            if ch.is_uppercase() && !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
            current.push(ch);
        }
        if !current.is_empty() {
            tokens.push(current);
        }
    }

    tokens
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bash::BashTool;
    use crate::tool::InterruptBehavior;
    use crate::{
        EnterPlanModeTool, ExitPlanModeTool, FileEditTool, FileReadTool, FileWriteTool, GlobTool,
        GrepTool, MonitorTool, TodoWriteTool, ToolSearchTool, WebFetchTool, WebSearchTool,
    };

    fn schema_contract_registry() -> Arc<ToolRegistry> {
        Arc::new_cyclic(|weak| {
            let registry = ToolRegistry::new();
            registry.register(BashTool::new());
            registry.register(FileReadTool::new());
            registry.register(FileEditTool::new());
            registry.register(FileWriteTool::new());
            registry.register(GlobTool::new());
            registry.register(GrepTool::new());
            registry.register(WebFetchTool::new());
            registry.register(WebSearchTool::new());
            registry.register(ToolSearchTool::new(weak.clone()));
            registry
        })
    }

    fn schema_string_array(schema: &serde_json::Value, key: &str) -> Vec<String> {
        let mut values = schema
            .get(key)
            .and_then(|value| value.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        values.sort();
        values
    }

    fn schema_property_names(schema: &serde_json::Value) -> Vec<String> {
        let mut names = schema
            .get("properties")
            .and_then(|value| value.as_object())
            .map(|properties| properties.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        names.sort();
        names
    }

    fn tool_schema_summary(registry: &ToolRegistry) -> String {
        registry
            .list()
            .into_iter()
            .map(|tool| {
                let required = schema_string_array(&tool.info.input_schema, "required").join(",");
                let properties = schema_property_names(&tool.info.input_schema).join(",");
                format!(
                    "{}|deferred:{}|required:[{}]|properties:[{}]",
                    tool.info.name, tool.should_defer, required, properties
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    struct BlockingTool;

    #[async_trait::async_trait]
    impl Tool for BlockingTool {
        fn info(&self) -> ToolInfo {
            ToolInfo {
                name: "BlockingTool".to_string(),
                description: "Blocking tool".to_string(),
                input_schema: serde_json::json!({}),
            }
        }

        fn interrupt_behavior(&self) -> InterruptBehavior {
            InterruptBehavior::Block
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            context: ToolContext,
        ) -> Result<rust_claude_core::tool_types::ToolResult, ToolError> {
            Ok(rust_claude_core::tool_types::ToolResult::success(
                context.tool_use_id,
                "ok".to_string(),
            ))
        }
    }

    #[test]
    fn test_schema_contract_summary_for_builtin_tools() {
        let registry = schema_contract_registry();

        let summary = tool_schema_summary(&registry);

        let expected = [
            "Bash|deferred:false|required:[command]|properties:[command,timeout_ms,workdir]",
            "FileEdit|deferred:false|required:[new_string,old_string]|properties:[file_path,new_string,old_string,path,replace_all]",
            "FileRead|deferred:false|required:[]|properties:[file_path,limit,offset,path]",
            "FileWrite|deferred:false|required:[content]|properties:[content,file_path,path]",
            "Glob|deferred:false|required:[pattern]|properties:[path,pattern]",
            "Grep|deferred:false|required:[pattern]|properties:[-A,-B,-C,-i,glob,head_limit,output_mode,path,pattern,type]",
            "ToolSearch|deferred:false|required:[query]|properties:[max_results,query]",
            "WebFetch|deferred:true|required:[url]|properties:[prompt,url]",
            "WebSearch|deferred:true|required:[query]|properties:[allowed_domains,blocked_domains,query]",
        ]
        .join("\n");
        assert_eq!(summary, expected);
    }

    #[test]
    fn test_schema_contract_file_tools_expose_path_aliases() {
        let registry = schema_contract_registry();

        for tool_name in ["FileRead", "FileEdit", "FileWrite"] {
            let tool = registry.get(tool_name).unwrap();
            let properties = schema_property_names(&tool.info.input_schema);
            assert!(properties.contains(&"path".to_string()));
            assert!(properties.contains(&"file_path".to_string()));
            assert!(tool.info.input_schema.get("anyOf").is_some());
        }
    }

    #[test]
    fn test_register_and_get() {
        let registry = ToolRegistry::new();
        registry.register(BashTool::new());

        let tool = registry.get("Bash").unwrap();
        assert_eq!(tool.info.name, "Bash");
        assert!(!tool.is_concurrency_safe);
        assert_eq!(tool.interrupt_behavior, InterruptBehavior::Cancel);
        assert!(registry.get("Unknown").is_none());
    }

    #[test]
    fn test_register_preserves_explicit_interrupt_behavior() {
        let registry = ToolRegistry::new();
        registry.register(BlockingTool);

        let tool = registry.get("BlockingTool").unwrap();
        assert_eq!(tool.interrupt_behavior, InterruptBehavior::Block);
    }

    #[test]
    fn test_list_sorted() {
        let registry = ToolRegistry::new();
        registry.register(BashTool::new());
        {
            let mut map = registry.tools.write().unwrap();
            map.insert(
                "FileWrite".to_string(),
                RegisteredTool {
                    info: ToolInfo {
                        name: "FileWrite".to_string(),
                        description: "Write file".to_string(),
                        input_schema: serde_json::json!({}),
                    },
                    is_read_only: false,
                    is_concurrency_safe: false,
                    should_defer: false,
                    interrupt_behavior: InterruptBehavior::Cancel,
                    tool: Arc::new(BashTool::new()),
                },
            );
        }

        let names = registry.names();
        assert_eq!(names, vec!["Bash", "FileWrite"]);
    }

    #[tokio::test]
    async fn test_schema_contract_tool_search_exposes_deferred_schema() {
        let registry = schema_contract_registry();
        let tool = registry.get("ToolSearch").unwrap();
        let result = tool
            .tool
            .execute(
                serde_json::json!({ "query": "select:WebFetch" }),
                ToolContext {
                    tool_use_id: "tool_schema".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let value: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        let entries = value.as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "WebFetch");
        assert!(entries[0]["description"].is_string());
        assert_eq!(entries[0]["input_schema"]["required"][0], "url");
    }

    #[tokio::test]
    async fn test_schema_contract_tool_search_excludes_non_deferred_tools() {
        let registry = schema_contract_registry();
        let tool = registry.get("ToolSearch").unwrap();
        let result = tool
            .tool
            .execute(
                serde_json::json!({ "query": "select:Bash" }),
                ToolContext {
                    tool_use_id: "tool_schema".to_string(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(result.content, "[]");
    }

    #[tokio::test]
    async fn test_execute_registered_tool() {
        let registry = ToolRegistry::new();
        registry.register(BashTool::new());

        let result = registry
            .execute(
                "Bash",
                serde_json::json!({ "command": "printf hello" }),
                ToolContext {
                    tool_use_id: "tool_1".to_string(),
                    app_state: None,
                    agent_context: None,
                    user_question_callback: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.content, "hello");
    }

    #[test]
    fn test_register_all_core_tools() {
        let registry = ToolRegistry::new();
        registry.register(BashTool::new());
        registry.register(EnterPlanModeTool::new());
        registry.register(ExitPlanModeTool::new());
        registry.register(FileReadTool::new());
        registry.register(FileEditTool::new());
        registry.register(FileWriteTool::new());
        registry.register(GlobTool::new());
        registry.register(GrepTool::new());
        registry.register(MonitorTool::new());
        registry.register(TodoWriteTool::new());

        let names = registry.names();
        assert_eq!(
            names,
            vec![
                "Bash",
                "EnterPlanMode",
                "ExitPlanMode",
                "FileEdit",
                "FileRead",
                "FileWrite",
                "Glob",
                "Grep",
                "Monitor",
                "TodoWrite"
            ]
        );
    }

    #[test]
    fn test_apply_tool_filters_matches_mcp_tool_names_exactly() {
        let registry = ToolRegistry::new();
        {
            let mut map = registry.tools.write().unwrap();
            map.insert(
                "mcp__remote__lookup".to_string(),
                RegisteredTool {
                    info: ToolInfo {
                        name: "mcp__remote__lookup".to_string(),
                        description: "Remote lookup".to_string(),
                        input_schema: serde_json::json!({}),
                    },
                    is_read_only: false,
                    is_concurrency_safe: false,
                    should_defer: true,
                    interrupt_behavior: InterruptBehavior::Cancel,
                    tool: Arc::new(BashTool::new()),
                },
            );
            map.insert(
                "mcp__other__lookup".to_string(),
                RegisteredTool {
                    info: ToolInfo {
                        name: "mcp__other__lookup".to_string(),
                        description: "Other lookup".to_string(),
                        input_schema: serde_json::json!({}),
                    },
                    is_read_only: false,
                    is_concurrency_safe: false,
                    should_defer: true,
                    interrupt_behavior: InterruptBehavior::Cancel,
                    tool: Arc::new(BashTool::new()),
                },
            );
        }

        registry.apply_tool_filters(&["mcp__remote__lookup".to_string()], &[]);

        assert_eq!(registry.names(), vec!["mcp__remote__lookup"]);
    }

    #[test]
    fn test_partition_deferred_and_non_deferred() {
        let registry = ToolRegistry::new();
        registry.register(BashTool::new());
        registry.register(WebSearchTool::new());

        let non_deferred = registry.get_non_deferred_tools();
        let deferred = registry.get_deferred_tools();

        assert_eq!(non_deferred.len(), 1);
        assert_eq!(non_deferred[0].info.name, "Bash");
        assert_eq!(deferred.len(), 1);
        assert_eq!(deferred[0].info.name, "WebSearch");
    }

    #[test]
    fn test_empty_registry_partitions() {
        let registry = ToolRegistry::new();
        assert!(registry.get_deferred_tools().is_empty());
        assert!(registry.get_non_deferred_tools().is_empty());
    }

    #[test]
    fn test_search_tools_exact_selection() {
        let registry = ToolRegistry::new();
        registry.register(WebSearchTool::new());
        registry.register(BashTool::new());

        let results = registry.search_tools("select:WebSearch", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].info.name, "WebSearch");

        let none = registry.search_tools("select:Bash", 5);
        assert!(none.is_empty()); // Bash is not deferred
    }

    #[test]
    fn test_search_tools_keyword_match() {
        let registry = ToolRegistry::new();
        registry.register(WebSearchTool::new());
        registry.register(WebFetchTool::new());
        registry.register(BashTool::new());

        let results = registry.search_tools("web", 5);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_tools_camelcase_tokenization() {
        let registry = ToolRegistry::new();
        registry.register(WebSearchTool::new());
        registry.register(BashTool::new());

        let results = registry.search_tools("search", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].info.name, "WebSearch");
    }

    #[test]
    fn test_search_tools_mcp_name_splitting() {
        let registry = ToolRegistry::new();
        {
            let mut map = registry.tools.write().unwrap();
            map.insert(
                "mcp__remote__lookup".to_string(),
                RegisteredTool {
                    info: ToolInfo {
                        name: "mcp__remote__lookup".to_string(),
                        description: "Remote lookup".to_string(),
                        input_schema: serde_json::json!({}),
                    },
                    is_read_only: false,
                    is_concurrency_safe: false,
                    should_defer: true,
                    interrupt_behavior: InterruptBehavior::Cancel,
                    tool: Arc::new(BashTool::new()),
                },
            );
        }

        let results = registry.search_tools("lookup", 5);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].info.name, "mcp__remote__lookup");
    }

    #[test]
    fn test_search_tools_scoring_name_over_description() {
        let registry = ToolRegistry::new();
        {
            let mut map = registry.tools.write().unwrap();
            map.insert(
                "WebSearch".to_string(),
                RegisteredTool {
                    info: ToolInfo {
                        name: "WebSearch".to_string(),
                        description: "Search the web".to_string(),
                        input_schema: serde_json::json!({}),
                    },
                    is_read_only: false,
                    is_concurrency_safe: false,
                    should_defer: true,
                    interrupt_behavior: InterruptBehavior::Cancel,
                    tool: Arc::new(BashTool::new()),
                },
            );
            map.insert(
                "OtherTool".to_string(),
                RegisteredTool {
                    info: ToolInfo {
                        name: "OtherTool".to_string(),
                        description: "A tool that can search things".to_string(),
                        input_schema: serde_json::json!({}),
                    },
                    is_read_only: false,
                    is_concurrency_safe: false,
                    should_defer: true,
                    interrupt_behavior: InterruptBehavior::Cancel,
                    tool: Arc::new(BashTool::new()),
                },
            );
        }

        let results = registry.search_tools("search", 5);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].info.name, "WebSearch"); // name match has higher score
    }

    #[test]
    fn test_estimate_deferred_schema_tokens() {
        let registry = ToolRegistry::new();
        registry.register(BashTool::new());
        registry.register(WebSearchTool::new());

        let tokens = registry.estimate_deferred_schema_tokens();
        // WebSearchTool is deferred; BashTool is not
        // The schema string length / 4 should be > 0 for WebSearchTool
        assert!(tokens > 0);
    }

    #[test]
    fn test_tokenize_tool_name() {
        assert_eq!(
            tokenize_tool_name("WebSearchTool"),
            vec!["Web", "Search", "Tool"]
        );
        assert_eq!(
            tokenize_tool_name("mcp__remote__lookup"),
            vec!["mcp", "remote", "lookup"]
        );
    }
}
