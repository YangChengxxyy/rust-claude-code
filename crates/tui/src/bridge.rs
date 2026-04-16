use rust_claude_core::compaction::CompactionResult;
use rust_claude_core::state::TodoItem;
use tokio::sync::{mpsc, oneshot};

use crate::events::{AppEvent, PermissionResponse};

/// Bridge used by the query loop to send events into the TUI.
#[derive(Debug, Clone)]
pub struct TuiBridge {
    event_tx: mpsc::Sender<AppEvent>,
}

impl TuiBridge {
    pub fn new(event_tx: mpsc::Sender<AppEvent>) -> Self {
        Self { event_tx }
    }

    pub async fn send_thinking_start(&self) {
        let _ = self.event_tx.send(AppEvent::ThinkingStart).await;
    }

    pub async fn send_thinking_delta(&self, text: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::ThinkingDelta(text.to_string()))
            .await;
    }

    pub async fn send_thinking_complete(&self, text: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::ThinkingComplete(text.to_string()))
            .await;
    }

    pub async fn send_stream_delta(&self, text: &str) {
        let _ = self.event_tx.send(AppEvent::StreamDelta(text.to_string())).await;
    }

    pub async fn send_stream_end(&self) {
        let _ = self.event_tx.send(AppEvent::StreamEnd).await;
    }

    pub async fn send_stream_cancelled(&self) {
        let _ = self.event_tx.send(AppEvent::StreamCancelled).await;
    }

    pub async fn send_tool_use(&self, name: &str, input: &serde_json::Value) {
        let _ = self
            .event_tx
            .send(AppEvent::ToolUseStart {
                name: name.to_string(),
                input: input.clone(),
            })
            .await;
    }

    pub async fn send_tool_result(&self, name: &str, output: &str, is_error: bool) {
        let _ = self
            .event_tx
            .send(AppEvent::ToolResult {
                name: name.to_string(),
                output: output.to_string(),
                is_error,
            })
            .await;
    }

    pub async fn send_usage_update(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
    ) {
        let _ = self
            .event_tx
            .send(AppEvent::UsageUpdate {
                input_tokens,
                output_tokens,
                cache_read_input_tokens,
                cache_creation_input_tokens,
            })
            .await;
    }

    pub async fn send_assistant_message(&self, text: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::AssistantMessage(text.to_string()))
            .await;
    }

    pub async fn send_status_update(
        &self,
        model: &str,
        model_setting: &str,
        permission_mode: &str,
    ) {
        let _ = self
            .event_tx
            .send(AppEvent::StatusUpdate {
                model: model.to_string(),
                model_setting: model_setting.to_string(),
                permission_mode: permission_mode.to_string(),
            })
            .await;
    }

    pub async fn send_error(&self, message: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::Error(message.to_string()))
            .await;
    }

    /// Request permission confirmation from the TUI user.
    /// Sends a PermissionRequest event and waits for the user's response.
    pub async fn request_permission(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Option<PermissionResponse> {
        let (tx, rx) = oneshot::channel();
        let send_result = self
            .event_tx
            .send(AppEvent::PermissionRequest {
                tool_name: tool_name.to_string(),
                input: input.clone(),
                response_tx: tx,
            })
            .await;

        if send_result.is_err() {
            return None;
        }

        rx.await.ok()
    }

    /// Notify the TUI that the todo list has been updated.
    pub async fn send_todo_update(&self, todos: Vec<TodoItem>) {
        let _ = self.event_tx.send(AppEvent::TodoUpdate(todos)).await;
    }

    /// Notify the TUI that compaction has started.
    pub async fn send_compaction_start(&self) {
        let _ = self.event_tx.send(AppEvent::CompactionStart).await;
    }

    /// Notify the TUI that compaction has completed.
    pub async fn send_compaction_complete(&self, result: CompactionResult) {
        let _ = self
            .event_tx
            .send(AppEvent::CompactionComplete { result })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bridge_sends_stream_delta() {
        let (tx, mut rx) = mpsc::channel(1);
        let bridge = TuiBridge::new(tx);

        bridge.send_stream_delta("hello").await;

        match rx.recv().await {
            Some(AppEvent::StreamDelta(text)) => assert_eq!(text, "hello"),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_bridge_sends_thinking_delta() {
        let (tx, mut rx) = mpsc::channel(1);
        let bridge = TuiBridge::new(tx);

        bridge.send_thinking_delta("reasoning").await;

        match rx.recv().await {
            Some(AppEvent::ThinkingDelta(text)) => assert_eq!(text, "reasoning"),
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_bridge_sends_tool_result() {
        let (tx, mut rx) = mpsc::channel(1);
        let bridge = TuiBridge::new(tx);

        bridge.send_tool_result("Bash", "ok", false).await;

        match rx.recv().await {
            Some(AppEvent::ToolResult { name, output, is_error }) => {
                assert_eq!(name, "Bash");
                assert_eq!(output, "ok");
                assert!(!is_error);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_bridge_request_permission() {
        let (tx, mut rx) = mpsc::channel(1);
        let bridge = TuiBridge::new(tx);

        let bridge_clone = bridge.clone();
        let handle = tokio::spawn(async move {
            bridge_clone
                .request_permission("Bash", &serde_json::json!({"command": "rm -rf /tmp"}))
                .await
        });

        match rx.recv().await {
            Some(AppEvent::PermissionRequest {
                tool_name,
                response_tx,
                ..
            }) => {
                assert_eq!(tool_name, "Bash");
                let _ = response_tx.send(PermissionResponse::Allow);
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let response = handle.await.unwrap();
        assert_eq!(response, Some(PermissionResponse::Allow));
    }

    #[tokio::test]
    async fn test_bridge_sends_todo_update() {
        use rust_claude_core::state::{TodoPriority, TodoStatus};

        let (tx, mut rx) = mpsc::channel(1);
        let bridge = TuiBridge::new(tx);

        bridge
            .send_todo_update(vec![TodoItem {
                id: "1".into(),
                content: "task".into(),
                status: TodoStatus::Pending,
                priority: TodoPriority::High,
            }])
            .await;

        match rx.recv().await {
            Some(AppEvent::TodoUpdate(todos)) => {
                assert_eq!(todos.len(), 1);
                assert_eq!(todos[0].id, "1");
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
