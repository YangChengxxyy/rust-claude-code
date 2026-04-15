use serde::{Deserialize, Serialize};

use crate::message::{Message, Usage};
use crate::permission::{PermissionCheck, PermissionMode, PermissionRequest, PermissionRule};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: TodoPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TodoPriority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSettings {
    pub model: String,
    pub system_prompt: Option<String>,
    pub max_tokens: u32,
    pub stream: bool,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub messages: Vec<Message>,
    pub todos: Vec<TodoItem>,
    pub permission_mode: PermissionMode,
    pub always_allow_rules: Vec<PermissionRule>,
    pub always_deny_rules: Vec<PermissionRule>,
    pub session: SessionSettings,
    pub cwd: std::path::PathBuf,
    pub total_usage: Usage,
}

impl AppState {
    pub fn new(cwd: std::path::PathBuf) -> Self {
        AppState {
            messages: Vec::new(),
            todos: Vec::new(),
            permission_mode: PermissionMode::Default,
            always_allow_rules: Vec::new(),
            always_deny_rules: Vec::new(),
            session: SessionSettings {
                model: "claude-sonnet-4-20250514".to_string(),
                system_prompt: None,
                max_tokens: 16384,
                stream: true,
            },
            cwd,
            total_usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        }
    }

    pub fn from_config(cwd: std::path::PathBuf, config: &crate::config::Config) -> Self {
        AppState {
            permission_mode: config.permission_mode,
            always_allow_rules: config.always_allow.clone(),
            always_deny_rules: config.always_deny.clone(),
            session: SessionSettings {
                model: config.model.clone(),
                system_prompt: config.system_prompt.clone(),
                max_tokens: config.max_tokens,
                stream: config.stream,
            },
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

    pub fn update_todos(&mut self, todos: Vec<TodoItem>) {
        let all_completed = todos.iter().all(|t| t.status == TodoStatus::Completed);
        if all_completed && !todos.is_empty() {
            self.todos.clear();
        } else {
            self.todos = todos;
        }
    }

    pub fn messages_for_api(&self) -> Vec<&Message> {
        self.messages.iter().collect()
    }

    pub fn check_permission(&self, request: PermissionRequest<'_>) -> PermissionCheck {
        self.permission_mode
            .check(request, &self.always_deny_rules, &self.always_allow_rules)
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
        assert!(state.todos.is_empty());
        assert_eq!(state.permission_mode, PermissionMode::Default);
        assert!(state.always_allow_rules.is_empty());
        assert!(state.always_deny_rules.is_empty());
        assert_eq!(state.session.model, "claude-sonnet-4-20250514");
        assert!(state.session.system_prompt.is_none());
        assert_eq!(state.session.max_tokens, 16384);
        assert!(state.session.stream);
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
    fn test_add_usage() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.add_usage(&Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });
        state.add_usage(&Usage {
            input_tokens: 200,
            output_tokens: 75,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });

        assert_eq!(state.total_usage.input_tokens, 300);
        assert_eq!(state.total_usage.output_tokens, 125);
    }

    #[test]
    fn test_update_todos_all_completed_clears() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.update_todos(vec![TodoItem {
            id: "1".to_string(),
            content: "task 1".to_string(),
            status: TodoStatus::Completed,
            priority: TodoPriority::High,
        }]);
        assert!(state.todos.is_empty());
    }

    #[test]
    fn test_update_todos_mixed_keeps() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.update_todos(vec![
            TodoItem {
                id: "1".to_string(),
                content: "task 1".to_string(),
                status: TodoStatus::Completed,
                priority: TodoPriority::High,
            },
            TodoItem {
                id: "2".to_string(),
                content: "task 2".to_string(),
                status: TodoStatus::InProgress,
                priority: TodoPriority::Medium,
            },
        ]);
        assert_eq!(state.todos.len(), 2);
    }

    #[test]
    fn test_todo_item_serde() {
        let item = TodoItem {
            id: "1".to_string(),
            content: "test task".to_string(),
            status: TodoStatus::InProgress,
            priority: TodoPriority::High,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: TodoItem = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, "1");
        assert_eq!(parsed.status, TodoStatus::InProgress);
        assert_eq!(parsed.priority, TodoPriority::High);
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
        };

        let state = AppState::from_config(std::path::PathBuf::from("/tmp"), &config);

        assert_eq!(state.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(state.session.model, "claude-test");
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
            system_prompt: Some("Be concise".to_string()),
            max_tokens: 4096,
            stream: false,
        };

        let json = serde_json::to_string(&settings).unwrap();
        let parsed: SessionSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.model, "claude-test");
        assert_eq!(parsed.system_prompt.as_deref(), Some("Be concise"));
        assert_eq!(parsed.max_tokens, 4096);
        assert!(!parsed.stream);
    }
}
