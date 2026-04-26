use std::pin::Pin;

use eventsource_stream::Eventsource;
use futures_core::Stream;
use futures_util::{stream, StreamExt, TryStreamExt};
use rust_claude_core::message::{ContentBlock, StopReason, Usage};
use serde::Deserialize;

use crate::error::ApiError;

pub type MessageStream = Pin<Box<dyn Stream<Item = Result<StreamEvent, ApiError>> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: String,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    MessageStart {
        message: StreamMessage,
    },
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentBlockDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDelta,
        #[serde(default)]
        usage: Option<Usage>,
    },
    MessageStop,
    Ping,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StreamMessage {
    pub id: String,
    pub role: rust_claude_core::message::Role,
    pub model: String,
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub stop_reason: Option<StopReason>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    TextDelta { text: String },
    InputJsonDelta { partial_json: String },
    ThinkingDelta { thinking: String },
    SignatureDelta { signature: String },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MessageDelta {
    #[serde(default)]
    pub stop_reason: Option<StopReason>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDeltaAccumulator {
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinkingDeltaAccumulator {
    thinking: String,
    signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolUseDeltaAccumulator {
    id: String,
    name: String,
    input_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentBlockAccumulator {
    Text(TextDeltaAccumulator),
    Thinking(ThinkingDeltaAccumulator),
    ToolUse(ToolUseDeltaAccumulator),
}

impl TextDeltaAccumulator {
    pub fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    pub fn push(&mut self, delta: &ContentBlockDelta) -> Result<(), ApiError> {
        match delta {
            ContentBlockDelta::TextDelta { text } => {
                self.text.push_str(text);
                Ok(())
            }
            other => Err(ApiError::Stream(format!(
                "text accumulator does not support delta: {other:?}"
            ))),
        }
    }

    pub fn into_content_block(self) -> ContentBlock {
        ContentBlock::text(self.text)
    }
}

impl ThinkingDeltaAccumulator {
    pub fn new() -> Self {
        Self {
            thinking: String::new(),
            signature: None,
        }
    }

    pub fn push(&mut self, delta: &ContentBlockDelta) -> Result<(), ApiError> {
        match delta {
            ContentBlockDelta::ThinkingDelta { thinking } => {
                self.thinking.push_str(thinking);
                Ok(())
            }
            ContentBlockDelta::SignatureDelta { signature } => {
                let sig = self.signature.get_or_insert_with(String::new);
                sig.push_str(signature);
                Ok(())
            }
            other => Err(ApiError::Stream(format!(
                "thinking accumulator does not support delta: {other:?}"
            ))),
        }
    }

    pub fn into_content_block(self) -> ContentBlock {
        match self.signature {
            Some(sig) => ContentBlock::thinking_with_signature(self.thinking, sig),
            None => ContentBlock::thinking(self.thinking),
        }
    }
}

impl ToolUseDeltaAccumulator {
    pub fn new(id: impl Into<String>, name: impl Into<String>, input: &serde_json::Value) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            input_json: match input {
                serde_json::Value::Object(map) if map.is_empty() => String::new(),
                other => other.to_string(),
            },
        }
    }

    pub fn push(&mut self, delta: &ContentBlockDelta) -> Result<(), ApiError> {
        match delta {
            ContentBlockDelta::InputJsonDelta { partial_json } => {
                self.input_json.push_str(partial_json);
                Ok(())
            }
            other => Err(ApiError::Stream(format!(
                "tool_use accumulator does not support delta: {other:?}"
            ))),
        }
    }

    pub fn into_content_block(self) -> Result<ContentBlock, ApiError> {
        let input = if self.input_json.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&self.input_json).map_err(ApiError::from)?
        };

        Ok(ContentBlock::tool_use(self.id, self.name, input))
    }
}

impl ContentBlockAccumulator {
    pub fn from_block(block: &ContentBlock) -> Result<Self, ApiError> {
        match block {
            ContentBlock::Text { .. } => Ok(Self::Text(TextDeltaAccumulator::new())),
            ContentBlock::Thinking { .. } => Ok(Self::Thinking(ThinkingDeltaAccumulator::new())),
            ContentBlock::ToolUse { id, name, input } => {
                Ok(Self::ToolUse(ToolUseDeltaAccumulator::new(id, name, input)))
            }
            other => Err(ApiError::Stream(format!(
                "content block accumulator does not support block: {other:?}"
            ))),
        }
    }

    pub fn push(&mut self, delta: &ContentBlockDelta) -> Result<(), ApiError> {
        match self {
            Self::Text(accumulator) => accumulator.push(delta),
            Self::Thinking(accumulator) => accumulator.push(delta),
            Self::ToolUse(accumulator) => accumulator.push(delta),
        }
    }

    pub fn into_content_block(self) -> Result<ContentBlock, ApiError> {
        match self {
            Self::Text(accumulator) => Ok(accumulator.into_content_block()),
            Self::Thinking(accumulator) => Ok(accumulator.into_content_block()),
            Self::ToolUse(accumulator) => accumulator.into_content_block(),
        }
    }
}

pub fn parse_sse_events(input: &str) -> Result<Vec<SseEvent>, ApiError> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut events = Vec::new();

    for chunk in normalized.split("\n\n") {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }

        let mut event_name = String::from("message");
        let mut data_lines = Vec::new();

        for line in chunk.lines() {
            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if let Some(value) = line.strip_prefix("event:") {
                event_name = value.trim_start().to_string();
                continue;
            }

            if let Some(value) = line.strip_prefix("data:") {
                data_lines.push(value.trim_start().to_string());
            }
        }

        if data_lines.is_empty() {
            continue;
        }

        events.push(SseEvent {
            event: event_name,
            data: data_lines.join("\n"),
        });
    }

    Ok(events)
}

pub fn parse_stream_event(event: &SseEvent) -> Result<StreamEvent, ApiError> {
    serde_json::from_str(&event.data).map_err(ApiError::from)
}

pub fn stream_events_from_text(input: &str) -> MessageStream {
    let events = match parse_sse_events(input) {
        Ok(events) => events,
        Err(error) => return Box::pin(stream::once(async move { Err(error) })),
    };

    Box::pin(stream::iter(
        events.into_iter().map(|event| parse_stream_event(&event)),
    ))
}

pub fn stream_events_from_response(response: reqwest::Response) -> MessageStream {
    let stream = response
        .bytes_stream()
        .map_err(map_reqwest_error)
        .eventsource()
        .map(|result| match result {
            Ok(event) => parse_stream_event(&SseEvent {
                event: event.event,
                data: event.data,
            }),
            Err(error) => Err(ApiError::Stream(error.to_string())),
        });

    Box::pin(stream)
}

fn map_reqwest_error(error: reqwest::Error) -> ApiError {
    if error.is_timeout() {
        ApiError::Timeout
    } else if error.is_connect() {
        ApiError::Connection(error.to_string())
    } else {
        ApiError::Http(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_events_extracts_event_and_data() {
        let input = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"role\":\"assistant\",\"model\":\"claude-sonnet-4-20250514\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0}}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n";

        let events = parse_sse_events(input).unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, "message_start");
        assert!(events[0].data.contains("\"type\":\"message_start\""));
        assert_eq!(events[1].event, "content_block_delta");
        assert!(events[1].data.contains("\"text\":\"Hel\""));
    }

    #[test]
    fn test_parse_sse_events_joins_multiline_data() {
        let input = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\ndata: \"index\":0,\ndata: \"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";

        let events = parse_sse_events(input).unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "content_block_delta");
        assert_eq!(
            events[0].data,
            "{\"type\":\"content_block_delta\",\n\"index\":0,\n\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}"
        );
    }

    #[test]
    fn test_parse_stream_event_message_start() {
        let event = SseEvent {
            event: "message_start".to_string(),
            data: "{\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"role\":\"assistant\",\"model\":\"claude-sonnet-4-20250514\",\"content\":[],\"usage\":{\"input_tokens\":1,\"output_tokens\":0,\"cache_creation_input_tokens\":0,\"cache_read_input_tokens\":0}}}".to_string(),
        };

        let parsed = parse_stream_event(&event).unwrap();
        assert!(matches!(parsed, StreamEvent::MessageStart { .. }));
    }

    #[tokio::test]
    async fn test_stream_events_from_text_yields_stream_events() {
        let input = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\nevent: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n";

        let events: Vec<_> = stream_events_from_text(input).collect().await;

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            Ok(StreamEvent::ContentBlockDelta { .. })
        ));
        assert!(matches!(
            &events[1],
            Ok(StreamEvent::ContentBlockDelta { .. })
        ));
    }

    #[test]
    fn test_text_delta_accumulator_builds_complete_block() {
        let mut accumulator = TextDeltaAccumulator::new();

        accumulator
            .push(&ContentBlockDelta::TextDelta {
                text: "Hel".to_string(),
            })
            .unwrap();
        accumulator
            .push(&ContentBlockDelta::TextDelta {
                text: "lo".to_string(),
            })
            .unwrap();

        assert_eq!(
            accumulator.into_content_block(),
            ContentBlock::text("Hello")
        );
    }

    #[test]
    fn test_text_delta_accumulator_rejects_non_text_delta() {
        let mut accumulator = TextDeltaAccumulator::new();

        let error = accumulator
            .push(&ContentBlockDelta::InputJsonDelta {
                partial_json: "{\"a\":1".to_string(),
            })
            .unwrap_err();

        assert!(
            matches!(error, ApiError::Stream(message) if message.contains("text accumulator does not support delta"))
        );
    }

    #[test]
    fn test_content_block_accumulator_builds_text_block_from_deltas() {
        let mut accumulator = ContentBlockAccumulator::from_block(&ContentBlock::text("")).unwrap();

        accumulator
            .push(&ContentBlockDelta::TextDelta {
                text: "Hel".to_string(),
            })
            .unwrap();
        accumulator
            .push(&ContentBlockDelta::TextDelta {
                text: "lo".to_string(),
            })
            .unwrap();

        assert_eq!(
            accumulator.into_content_block().unwrap(),
            ContentBlock::text("Hello")
        );
    }

    #[test]
    fn test_content_block_accumulator_builds_thinking_block_from_deltas() {
        let mut accumulator =
            ContentBlockAccumulator::from_block(&ContentBlock::thinking("")).unwrap();

        accumulator
            .push(&ContentBlockDelta::ThinkingDelta {
                thinking: "plan ".to_string(),
            })
            .unwrap();
        accumulator
            .push(&ContentBlockDelta::ThinkingDelta {
                thinking: "carefully".to_string(),
            })
            .unwrap();

        assert_eq!(
            accumulator.into_content_block().unwrap(),
            ContentBlock::thinking("plan carefully")
        );
    }

    #[test]
    fn test_content_block_accumulator_builds_tool_use_block_from_json_deltas() {
        let mut accumulator = ContentBlockAccumulator::from_block(&ContentBlock::tool_use(
            "tool_1",
            "Bash",
            serde_json::json!({}),
        ))
        .unwrap();

        accumulator
            .push(&ContentBlockDelta::InputJsonDelta {
                partial_json: "{\"command\":\"pw".to_string(),
            })
            .unwrap();
        accumulator
            .push(&ContentBlockDelta::InputJsonDelta {
                partial_json: "d\"}".to_string(),
            })
            .unwrap();

        assert_eq!(
            accumulator.into_content_block().unwrap(),
            ContentBlock::tool_use("tool_1", "Bash", serde_json::json!({ "command": "pwd" }))
        );
    }
}
