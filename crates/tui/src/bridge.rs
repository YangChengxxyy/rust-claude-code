use tokio::sync::mpsc;

use crate::events::AppEvent;

/// Bridge used by the query loop to send events into the TUI.
#[derive(Debug, Clone)]
pub struct TuiBridge {
    event_tx: mpsc::Sender<AppEvent>,
}

impl TuiBridge {
    pub fn new(event_tx: mpsc::Sender<AppEvent>) -> Self {
        Self { event_tx }
    }

    pub async fn send_stream_delta(&self, text: &str) {
        let _ = self.event_tx.send(AppEvent::StreamDelta(text.to_string())).await;
    }

    pub async fn send_stream_end(&self) {
        let _ = self.event_tx.send(AppEvent::StreamEnd).await;
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

    pub async fn send_usage_update(&self, input_tokens: u64, output_tokens: u64) {
        let _ = self
            .event_tx
            .send(AppEvent::UsageUpdate {
                input_tokens,
                output_tokens,
            })
            .await;
    }

    pub async fn send_assistant_message(&self, text: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::AssistantMessage(text.to_string()))
            .await;
    }

    pub async fn send_error(&self, message: &str) {
        let _ = self
            .event_tx
            .send(AppEvent::Error(message.to_string()))
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
}
