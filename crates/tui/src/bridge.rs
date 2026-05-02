use rust_claude_core::compaction::CompactionResult;
use rust_claude_core::config::ConfigProvenance;
use rust_claude_core::session::{ContextSnapshot, SessionSummary};
use rust_claude_core::state::TodoItem;
use rust_claude_sdk::output::{OutputSink, PermissionDecision, PermissionUI, UserQuestionUI};
use rust_claude_tools::{AskUserQuestionRequest, AskUserQuestionResponse};
use tokio::sync::{mpsc, oneshot};

use crate::events::{format_provenance_summary, AppEvent, ChatMessage, PermissionResponse};

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

    pub async fn send_stream_start(&self) {
        let _ = self.event_tx.send(AppEvent::StreamStart).await;
    }

    pub async fn send_stream_delta(&self, text: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::StreamDelta(text.to_string()))
            .await;
    }

    pub async fn send_stream_end(&self) {
        let _ = self.event_tx.send(AppEvent::StreamEnd).await;
    }

    pub async fn send_stream_cancelled(&self) {
        let _ = self.event_tx.send(AppEvent::StreamCancelled).await;
    }

    pub async fn send_tool_input_stream_start(&self, name: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::ToolInputStreamStart {
                name: name.to_string(),
            })
            .await;
    }

    pub async fn send_tool_input_delta(&self, name: &str, fragment: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::ToolInputDelta {
                name: name.to_string(),
                json_fragment: fragment.to_string(),
            })
            .await;
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
        git_branch: Option<&str>,
    ) {
        let _ = self
            .event_tx
            .send(AppEvent::StatusUpdate {
                model: model.to_string(),
                model_setting: model_setting.to_string(),
                permission_mode: permission_mode.to_string(),
                git_branch: git_branch.map(str::to_string),
            })
            .await;
    }

    pub async fn send_config_info(&self, provenance: &ConfigProvenance) {
        let (model_source, permission_source, base_url_source, theme_source) =
            format_provenance_summary(provenance);
        let _ = self
            .event_tx
            .send(AppEvent::ConfigInfo {
                model_source,
                permission_source,
                base_url_source,
                theme_source,
            })
            .await;
    }

    pub async fn send_session_list(&self, sessions: Vec<SessionSummary>, skipped: usize) {
        let _ = self
            .event_tx
            .send(AppEvent::SessionList { sessions, skipped })
            .await;
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn send_session_resumed(
        &self,
        summary: SessionSummary,
        messages: Vec<ChatMessage>,
        model: String,
        model_setting: String,
        permission_mode: String,
        git_branch: Option<String>,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
    ) {
        let _ = self
            .event_tx
            .send(AppEvent::SessionResumed {
                summary,
                messages,
                model,
                model_setting,
                permission_mode,
                git_branch,
                input_tokens,
                output_tokens,
                cache_read_input_tokens,
                cache_creation_input_tokens,
            })
            .await;
    }

    pub async fn send_context_snapshot(&self, snapshot: ContextSnapshot) {
        let _ = self
            .event_tx
            .send(AppEvent::ContextSnapshot(snapshot))
            .await;
    }

    pub async fn send_conversation_replaced(
        &self,
        messages: Vec<ChatMessage>,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
        notice: String,
    ) {
        let _ = self
            .event_tx
            .send(AppEvent::ConversationReplaced {
                messages,
                input_tokens,
                output_tokens,
                cache_read_input_tokens,
                cache_creation_input_tokens,
                notice,
            })
            .await;
    }

    pub async fn send_error(&self, message: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::Error(message.to_string()))
            .await;
    }

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

    pub async fn request_user_question(
        &self,
        request: AskUserQuestionRequest,
    ) -> Option<AskUserQuestionResponse> {
        let (tx, rx) = oneshot::channel();
        let send_result = self
            .event_tx
            .send(AppEvent::UserQuestionRequest {
                request,
                response_tx: tx,
            })
            .await;

        if send_result.is_err() {
            return None;
        }

        rx.await.ok().flatten()
    }

    pub async fn send_todo_update(&self, todos: Vec<TodoItem>) {
        let _ = self.event_tx.send(AppEvent::TodoUpdate(todos)).await;
    }

    pub async fn send_compaction_start(&self) {
        let _ = self.event_tx.send(AppEvent::CompactionStart).await;
    }

    pub async fn send_compaction_complete(&self, result: CompactionResult) {
        let _ = self
            .event_tx
            .send(AppEvent::CompactionComplete { result })
            .await;
    }

    pub async fn send_hook_blocked(&self, tool_name: &str, reason: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::HookBlocked {
                tool_name: tool_name.to_string(),
                reason: reason.to_string(),
            })
            .await;
    }
}

// Implement SDK traits on TuiBridge
impl OutputSink for TuiBridge {
    fn stream_start(&self) {
        let _ = self.event_tx.try_send(AppEvent::StreamStart);
    }

    fn stream_delta(&self, text: &str) {
        let _ = self
            .event_tx
            .try_send(AppEvent::StreamDelta(text.to_string()));
    }

    fn stream_end(&self) {
        let _ = self.event_tx.try_send(AppEvent::StreamEnd);
    }

    fn thinking_start(&self) {
        let _ = self.event_tx.try_send(AppEvent::ThinkingStart);
    }

    fn thinking_delta(&self, text: &str) {
        let _ = self
            .event_tx
            .try_send(AppEvent::ThinkingDelta(text.to_string()));
    }

    fn thinking_complete(&self, text: &str) {
        let _ = self
            .event_tx
            .try_send(AppEvent::ThinkingComplete(text.to_string()));
    }

    fn tool_input_start(&self, name: &str) {
        let _ = self.event_tx.try_send(AppEvent::ToolInputStreamStart {
            name: name.to_string(),
        });
    }

    fn tool_input_delta(&self, name: &str, json_fragment: &str) {
        let _ = self.event_tx.try_send(AppEvent::ToolInputDelta {
            name: name.to_string(),
            json_fragment: json_fragment.to_string(),
        });
    }

    fn tool_use(&self, name: &str, input: &serde_json::Value) {
        let _ = self.event_tx.try_send(AppEvent::ToolUseStart {
            name: name.to_string(),
            input: input.clone(),
        });
    }

    fn tool_result(&self, name: &str, output: &str, is_error: bool) {
        let _ = self.event_tx.try_send(AppEvent::ToolResult {
            name: name.to_string(),
            output: output.to_string(),
            is_error,
        });
    }

    fn usage(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
    ) {
        let _ = self.event_tx.try_send(AppEvent::UsageUpdate {
            input_tokens,
            output_tokens,
            cache_read_input_tokens,
            cache_creation_input_tokens,
        });
    }

    fn error(&self, message: &str) {
        let _ = self
            .event_tx
            .try_send(AppEvent::Error(message.to_string()));
    }

    fn compaction_start(&self) {
        let _ = self.event_tx.try_send(AppEvent::CompactionStart);
    }

    fn compaction_complete(&self, result: &CompactionResult) {
        let _ = self
            .event_tx
            .try_send(AppEvent::CompactionComplete {
                result: result.clone(),
            });
    }

    fn hook_blocked(&self, tool_name: &str, reason: &str) {
        let _ = self.event_tx.try_send(AppEvent::HookBlocked {
            tool_name: tool_name.to_string(),
            reason: reason.to_string(),
        });
    }

    fn todo_update(&self, todos: &[TodoItem]) {
        let _ = self.event_tx.try_send(AppEvent::TodoUpdate(todos.to_vec()));
    }
}

#[async_trait::async_trait]
impl PermissionUI for TuiBridge {
    async fn request(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Option<PermissionDecision> {
        self.request_permission(tool_name, input)
            .await
            .map(|resp| match resp {
                crate::events::PermissionResponse::Allow => PermissionDecision::Allow,
                crate::events::PermissionResponse::AlwaysAllow => PermissionDecision::AllowAlways,
                crate::events::PermissionResponse::Deny => PermissionDecision::Deny,
                crate::events::PermissionResponse::AlwaysDeny => PermissionDecision::DenyAlways,
            })
    }
}

#[async_trait::async_trait]
impl UserQuestionUI for TuiBridge {
    async fn ask(
        &self,
        request: AskUserQuestionRequest,
    ) -> Option<AskUserQuestionResponse> {
        self.request_user_question(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::state::{TodoPriority, TodoStatus};

    #[tokio::test]
    async fn test_bridge_sends_stream_start() {
        let (tx, mut rx) = mpsc::channel(1);
        let bridge = TuiBridge::new(tx);

        bridge.send_stream_start().await;

        match rx.recv().await {
            Some(AppEvent::StreamStart) => {}
            other => panic!("unexpected event: {other:?}"),
        }
    }

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
    async fn test_bridge_request_user_question() {
        let (tx, mut rx) = mpsc::channel(1);
        let bridge = TuiBridge::new(tx);

        let bridge_clone = bridge.clone();
        let handle = tokio::spawn(async move {
            bridge_clone
                .request_user_question(AskUserQuestionRequest {
                    question: "Pick one".into(),
                    options: vec![],
                    allow_custom: true,
                })
                .await
        });

        match rx.recv().await {
            Some(AppEvent::UserQuestionRequest {
                request,
                response_tx,
            }) => {
                assert_eq!(request.question, "Pick one");
                let _ = response_tx.send(Some(AskUserQuestionResponse {
                    selected_label: None,
                    answer: "custom".into(),
                    custom: true,
                }));
            }
            other => panic!("unexpected event: {other:?}"),
        }

        let response = handle.await.unwrap();
        assert_eq!(
            response,
            Some(AskUserQuestionResponse {
                selected_label: None,
                answer: "custom".into(),
                custom: true,
            })
        );
    }

    #[tokio::test]
    async fn test_bridge_sends_todo_update() {
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
