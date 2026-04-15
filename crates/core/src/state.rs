use serde::{Deserialize, Serialize};

use crate::message::{Message, Usage};
use crate::permission::PermissionMode;

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

#[derive(Debug, Clone)]
pub struct AppState {
    pub messages: Vec<Message>,
    pub todos: Vec<TodoItem>,
    pub permission_mode: PermissionMode,
    pub model: String,
    pub max_tokens: u32,
    pub cwd: std::path::PathBuf,
    pub total_usage: Usage,
}

impl AppState {
    pub fn new(cwd: std::path::PathBuf) -> Self {
        AppState {
            messages: Vec::new(),
            todos: Vec::new(),
            permission_mode: PermissionMode::Default,
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 16384,
            cwd,
            total_usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
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
        assert_eq!(state.model, "claude-sonnet-4-20250514");
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
}
