use serde::{Deserialize, Serialize};

use crate::config::{Config, ConfigProvenance};
use crate::git::GitContextSnapshot;
use crate::message::{Message, Usage};
use crate::permission::{PermissionCheck, PermissionMode, PermissionRequest, PermissionRule};

/// A task with status tracking. Replaces the previous TodoItem type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub content: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    High,
    Medium,
    Low,
}

// Backward-compatible type aliases for existing code (TUI, TodoWriteTool, etc.)
pub type TodoItem = Task;
pub type TodoStatus = TaskStatus;
pub type TodoPriority = TaskPriority;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSettings {
    pub model: String,
    pub model_setting: String,
    pub system_prompt: Option<String>,
    pub max_tokens: u32,
    pub stream: bool,
    #[serde(default = "default_thinking_enabled")]
    pub thinking_enabled: bool,
}

fn default_thinking_enabled() -> bool {
    true
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub messages: Vec<Message>,
    pub tasks: Vec<Task>,
    pub permission_mode: PermissionMode,
    pub always_allow_rules: Vec<PermissionRule>,
    pub always_deny_rules: Vec<PermissionRule>,
    pub session: SessionSettings,
    pub cwd: std::path::PathBuf,
    pub total_usage: Usage,
    pub config: Config,
    pub config_provenance: ConfigProvenance,
    pub git_context: Option<GitContextSnapshot>,
    /// Usage from the most recent API response (for accurate token counting).
    pub last_api_usage: Option<Usage>,
    /// Message count at the time of the last API response.
    pub last_api_message_index: usize,
}

impl AppState {
    pub fn new(cwd: std::path::PathBuf) -> Self {
        AppState {
            messages: Vec::new(),
            tasks: Vec::new(),
            permission_mode: PermissionMode::Default,
            always_allow_rules: Vec::new(),
            always_deny_rules: Vec::new(),
            session: SessionSettings {
                model: "claude-sonnet-4-20250514".to_string(),
                model_setting: "claude-sonnet-4-20250514".to_string(),
                system_prompt: None,
                max_tokens: 16384,
                stream: true,
                thinking_enabled: true,
            },
            cwd,
            total_usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
            config: Config::with_credential(String::new(), false),
            config_provenance: ConfigProvenance::default(),
            git_context: None,
            last_api_usage: None,
            last_api_message_index: 0,
        }
    }

    pub fn from_config(cwd: std::path::PathBuf, config: &crate::config::Config) -> Self {
        AppState {
            permission_mode: config.permission_mode,
            always_allow_rules: config.always_allow.clone(),
            always_deny_rules: config.always_deny.clone(),
            session: SessionSettings {
                model: config.model.clone(),
                model_setting: config.model.clone(),
                system_prompt: config.system_prompt.clone(),
                max_tokens: config.max_tokens,
                stream: config.stream,
                thinking_enabled: true,
            },
            config: config.clone(),
            config_provenance: config.provenance.clone(),
            ..Self::new(cwd)
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn add_usage(&mut self, usage: &Usage) {
        self.total_usage.input_tokens += usage.input_tokens;
        self.total_usage.output_tokens += usage.output_tokens;
        self.total_usage.cache_creation_input_tokens += usage.cache_creation_input_tokens;
        self.total_usage.cache_read_input_tokens += usage.cache_read_input_tokens;
    }

    pub fn add_assistant_message(&mut self, message: Message) {
        if let Some(usage) = &message.usage {
            self.add_usage(usage);
        }
        self.messages.push(message);
    }

    pub fn most_recent_assistant_usage(&self) -> Option<&Usage> {
        self.messages
            .iter()
            .rev()
            .find(|message| matches!(message.role, crate::message::Role::Assistant))
            .and_then(|message| message.usage.as_ref())
    }

    /// Update the task list. If all tasks are completed or cancelled, clear the list.
    pub fn update_tasks(&mut self, tasks: Vec<Task>) {
        let all_done = tasks.iter().all(|t| {
            matches!(t.status, TaskStatus::Completed | TaskStatus::Cancelled)
        });
        if all_done && !tasks.is_empty() {
            self.tasks.clear();
        } else {
            self.tasks = tasks;
        }
    }

    /// Backward-compatible alias for update_tasks.
    pub fn update_todos(&mut self, todos: Vec<TodoItem>) {
        self.update_tasks(todos);
    }

    pub fn messages_for_api(&self) -> Vec<&Message> {
        self.messages.iter().collect()
    }

    pub fn check_permission(&self, request: PermissionRequest<'_>) -> PermissionCheck {
        self.permission_mode
            .check(request, &self.always_deny_rules, &self.always_allow_rules)
    }

    /// Record API usage from the most recent response, for usage-based token counting.
    pub fn update_api_usage(&mut self, usage: Usage) {
        self.last_api_usage = Some(usage);
        self.last_api_message_index = self.messages.len();
    }

    pub fn conversation_turns(&self) -> usize {
        self.messages
            .iter()
            .filter(|m| matches!(m.role, crate::message::Role::Assistant))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new(std::path::PathBuf::from("/tmp"));
        assert!(state.messages.is_empty());
        assert!(state.tasks.is_empty());
        assert_eq!(state.permission_mode, PermissionMode::Default);
        assert!(state.always_allow_rules.is_empty());
        assert!(state.always_deny_rules.is_empty());
        assert_eq!(state.session.model, "claude-sonnet-4-20250514");
        assert_eq!(state.session.model_setting, "claude-sonnet-4-20250514");
        assert!(state.session.system_prompt.is_none());
        assert_eq!(state.session.max_tokens, 16384);
        assert!(state.session.stream);
        assert!(state.most_recent_assistant_usage().is_none());
        assert_eq!(state.config_provenance, ConfigProvenance::default());
        assert!(state.git_context.is_none());
    }

    #[test]
    fn test_add_message() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.add_message(Message::user("hello"));
        state.add_message(Message::assistant(vec![
            crate::message::ContentBlock::text("hi"),
        ]));

        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.conversation_turns(), 1);
    }

    #[test]
    fn test_add_assistant_message_updates_most_recent_usage() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        let usage = Usage {
            input_tokens: 150_000,
            output_tokens: 30_000,
            cache_creation_input_tokens: 25_000,
            cache_read_input_tokens: 0,
        };
        state.add_assistant_message(Message::assistant_with_usage(
            vec![crate::message::ContentBlock::text("hi")],
            usage.clone(),
        ));

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.most_recent_assistant_usage(), Some(&usage));
        assert_eq!(state.total_usage.input_tokens, 150_000);
    }

    #[test]
    fn test_update_tasks_all_completed_clears() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.update_tasks(vec![Task {
            id: "1".to_string(),
            content: "task 1".to_string(),
            status: TaskStatus::Completed,
            priority: TaskPriority::High,
        }]);
        assert!(state.tasks.is_empty());
    }

    #[test]
    fn test_update_tasks_all_cancelled_clears() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.update_tasks(vec![Task {
            id: "1".to_string(),
            content: "task 1".to_string(),
            status: TaskStatus::Cancelled,
            priority: TaskPriority::Low,
        }]);
        assert!(state.tasks.is_empty());
    }

    #[test]
    fn test_update_tasks_mixed_keeps() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.update_tasks(vec![
            Task {
                id: "1".to_string(),
                content: "task 1".to_string(),
                status: TaskStatus::Completed,
                priority: TaskPriority::High,
            },
            Task {
                id: "2".to_string(),
                content: "task 2".to_string(),
                status: TaskStatus::InProgress,
                priority: TaskPriority::Medium,
            },
        ]);
        assert_eq!(state.tasks.len(), 2);
    }

    #[test]
    fn test_task_serde() {
        let item = Task {
            id: "1".to_string(),
            content: "test task".to_string(),
            status: TaskStatus::InProgress,
            priority: TaskPriority::High,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "1");
        assert_eq!(parsed.status, TaskStatus::InProgress);
        assert_eq!(parsed.priority, TaskPriority::High);
    }

    #[test]
    fn test_task_cancelled_status_serde() {
        let item = Task {
            id: "1".to_string(),
            content: "cancelled task".to_string(),
            status: TaskStatus::Cancelled,
            priority: TaskPriority::Low,
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"cancelled\""));
        let parsed: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, TaskStatus::Cancelled);
    }

    #[test]
    fn test_backward_compat_todo_aliases() {
        // Verify type aliases work
        let item: TodoItem = Task {
            id: "1".to_string(),
            content: "test".to_string(),
            status: TodoStatus::Pending,
            priority: TodoPriority::Medium,
        };
        assert_eq!(item.status, TaskStatus::Pending);
    }

    #[test]
    fn test_check_permission_uses_state_rules() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.always_deny_rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git status".to_string()),
            rule_type: crate::permission::RuleType::Deny,
        }];
        state.always_allow_rules = vec![PermissionRule {
            tool_name: "Bash".to_string(),
            pattern: Some("git *".to_string()),
            rule_type: crate::permission::RuleType::Allow,
        }];

        let check = state.check_permission(PermissionRequest {
            tool_name: "Bash",
            command: Some("git status"),
            is_read_only: false,
        });

        assert!(matches!(check, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_from_config_copies_permission_and_model_settings() {
        let config = crate::config::Config {
            api_key: "test-key".to_string(),
            model: "claude-test".to_string(),
            base_url: None,
            bearer_auth: false,
            system_prompt: Some("You are a test assistant".to_string()),
            max_tokens: 2048,
            permission_mode: PermissionMode::AcceptEdits,
            always_allow: vec![PermissionRule {
                tool_name: "FileEdit".to_string(),
                pattern: None,
                rule_type: crate::permission::RuleType::Allow,
            }],
            always_deny: vec![PermissionRule {
                tool_name: "Bash".to_string(),
                pattern: None,
                rule_type: crate::permission::RuleType::Deny,
            }],
            stream: true,
            theme: crate::config::Theme::Dark,
            provenance: crate::config::ConfigProvenance::default(),
        };

        let state = AppState::from_config(std::path::PathBuf::from("/tmp"), &config);

        assert_eq!(state.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(state.session.model, "claude-test");
        assert_eq!(state.session.model_setting, "claude-test");
        assert_eq!(
            state.session.system_prompt.as_deref(),
            Some("You are a test assistant")
        );
        assert_eq!(state.session.max_tokens, 2048);
        assert!(state.session.stream);
        assert_eq!(state.always_allow_rules, config.always_allow);
        assert_eq!(state.always_deny_rules, config.always_deny);
    }

    #[test]
    fn test_session_settings_serde() {
        let settings = SessionSettings {
            model: "claude-test".to_string(),
            model_setting: "claude-test".to_string(),
            system_prompt: Some("Be concise".to_string()),
            max_tokens: 4096,
            stream: false,
            thinking_enabled: true,
        };

        let json = serde_json::to_string(&settings).unwrap();
        let parsed: SessionSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.model, "claude-test");
        assert_eq!(parsed.model_setting, "claude-test");
        assert_eq!(parsed.system_prompt.as_deref(), Some("Be concise"));
        assert_eq!(parsed.max_tokens, 4096);
        assert!(!parsed.stream);
    }
}
