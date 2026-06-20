use rust_claude_core::compaction::CompactionResult;
use rust_claude_core::message::Message;
use rust_claude_core::session::SessionEvent;
use rust_claude_core::state::TodoItem;
use serde_json::json;
use std::io::Write;
use std::sync::{Arc, Mutex};
use rust_claude_tools::{AskUserQuestionRequest, AskUserQuestionResponse};

/// Decision returned by PermissionUI when a tool requires interactive confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    AllowAlways,
    Deny,
    DenyAlways,
}

/// Sink for streaming output events from the agent loop.
///
/// Implementations can forward these to a terminal UI, log them,
/// or discard them in headless mode.
pub trait OutputSink: Send + Sync {
    fn stream_start(&self) {}
    fn stream_delta(&self, _text: &str) {}
    fn stream_end(&self) {}
    fn stream_cancelled(&self) {}

    fn thinking_start(&self) {}
    fn thinking_delta(&self, _text: &str) {}
    fn thinking_complete(&self, _text: &str) {}

    fn tool_input_start(&self, _name: &str) {}
    fn tool_input_delta(&self, _name: &str, _json_fragment: &str) {}

    fn tool_use(&self, _name: &str, _input: &serde_json::Value) {}
    fn tool_result(&self, _name: &str, _output: &str, _is_error: bool) {}

    /// Tool invocation including its unique tool-use id.
    ///
    /// Default delegates to [`OutputSink::tool_use`]. Sinks that correlate a
    /// tool call with its result by id (e.g. `StreamJsonOutputSink`) override
    /// this; sinks that ignore ids inherit the old behavior unchanged.
    fn tool_use_with_id(&self, _id: &str, name: &str, input: &serde_json::Value) {
        self.tool_use(name, input);
    }

    /// Tool result keyed by the originating tool-use id.
    ///
    /// Default delegates to [`OutputSink::tool_result`].
    fn tool_result_with_id(
        &self,
        _id: &str,
        name: &str,
        output: &str,
        is_error: bool,
    ) {
        self.tool_result(name, output, is_error);
    }

    fn usage(
        &self,
        _input_tokens: u64,
        _output_tokens: u64,
        _cache_read_input_tokens: u64,
        _cache_creation_input_tokens: u64,
    ) {
    }

    fn error(&self, _message: &str) {}

    fn compaction_start(&self) {}
    fn compaction_complete(&self, _result: &CompactionResult) {}

    fn hook_blocked(&self, _tool_name: &str, _reason: &str) {}

    fn todo_update(&self, _todos: &[TodoItem]) {}
}

/// UI for interactive permission confirmation.
#[async_trait::async_trait]
pub trait PermissionUI: Send + Sync {
    /// Request the user's decision for a tool invocation.
    ///
    /// Returns `None` if the UI is unavailable (e.g., headless mode).
    async fn request(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Option<PermissionDecision>;
}

/// UI for structured user questions (AskUserQuestionTool).
#[async_trait::async_trait]
pub trait UserQuestionUI: Send + Sync {
    /// Ask the user a structured question and await their response.
    ///
    /// Returns `None` if the UI is unavailable.
    async fn ask(&self, request: AskUserQuestionRequest) -> Option<AskUserQuestionResponse>;
}

// No-op implementations for headless mode

/// An OutputSink that discards all streaming events.
pub struct NoopOutputSink;

impl OutputSink for NoopOutputSink {}

/// NDJSON (newline-delimited JSON) output sink for headless / SDK usage.
///
/// Emits one JSON object per line so each line can be parsed independently by
/// `serde_json`. The event shape follows the stream-json contract:
///
/// ```jsonc
/// {"type":"message_start","session_id":"..."}
/// {"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}
/// {"type":"thinking_delta","text":"..."}
/// {"type":"tool_use","id":"...","name":"FileRead","input":{}}
/// {"type":"tool_result","tool_use_id":"...","name":"FileRead","is_error":false,"content":"..."}
/// {"type":"usage","input_tokens":1,"output_tokens":2,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}
/// {"type":"error","message":"..."}
/// {"type":"done"}
/// ```
///
/// The sink is cheaply cloneable (it shares one writer through an `Arc`), so a
/// caller can keep a handle to emit the terminal `done` / `error` after the run
/// finishes while passing another clone into the query loop as its
/// [`OutputSink`]. Both clones write to the same underlying stream.
///
/// `done` is not emitted by any [`OutputSink`] callback — the agent loop calls
/// `stream_end` once per assistant message, which is not the end of the whole
/// turn. The caller must invoke [`StreamJsonOutputSink::emit_done`] (or
/// [`StreamJsonOutputSink::emit_error`]) exactly once after the run completes.
#[derive(Clone)]
pub struct StreamJsonOutputSink {
    inner: Arc<StreamJsonInner>,
}

struct StreamJsonInner {
    session_id: String,
    writer: Mutex<Box<dyn Write + Send>>,
}

impl StreamJsonOutputSink {
    /// Create a sink that writes NDJSON lines to `writer`.
    pub fn new(session_id: impl Into<String>, writer: Box<dyn Write + Send>) -> Self {
        Self {
            inner: Arc::new(StreamJsonInner {
                session_id: session_id.into(),
                writer: Mutex::new(writer),
            }),
        }
    }

    fn emit(&self, value: &serde_json::Value) {
        let Ok(line) = serde_json::to_string(value) else {
            return;
        };
        let mut writer = self
            .inner
            .writer
            .lock()
            .expect("stream-json writer poisoned");
        let _ = writeln!(writer, "{line}");
        let _ = writer.flush();
    }

    /// Emit the terminal `done` event. Call exactly once after a successful run.
    pub fn emit_done(&self) {
        self.emit(&json!({"type": "done"}));
    }

    /// Emit an `error` event. Call for fatal run errors (then `emit_done`).
    pub fn emit_error(&self, message: &str) {
        self.emit(&json!({"type": "error", "message": message}));
    }
}

impl OutputSink for StreamJsonOutputSink {
    fn stream_start(&self) {
        self.emit(&json!({
            "type": "message_start",
            "session_id": self.inner.session_id,
        }));
    }

    fn stream_delta(&self, text: &str) {
        self.emit(&json!({
            "type": "content_block_delta",
            "delta": { "type": "text_delta", "text": text },
        }));
    }

    fn thinking_delta(&self, text: &str) {
        self.emit(&json!({"type": "thinking_delta", "text": text}));
    }

    fn tool_use_with_id(&self, id: &str, name: &str, input: &serde_json::Value) {
        self.emit(&json!({
            "type": "tool_use",
            "id": id,
            "name": name,
            "input": input,
        }));
    }

    fn tool_result_with_id(&self, id: &str, name: &str, output: &str, is_error: bool) {
        self.emit(&json!({
            "type": "tool_result",
            "tool_use_id": id,
            "name": name,
            "is_error": is_error,
            "content": output,
        }));
    }

    fn usage(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_input_tokens: u64,
        cache_creation_input_tokens: u64,
    ) {
        self.emit(&json!({
            "type": "usage",
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "cache_read_input_tokens": cache_read_input_tokens,
            "cache_creation_input_tokens": cache_creation_input_tokens,
        }));
    }

    fn error(&self, message: &str) {
        self.emit(&json!({"type": "error", "message": message}));
    }

    // stream_end / thinking_start / thinking_complete / tool_use / tool_result /
    // compaction_* / hook_blocked / todo_update: inherited no-ops. Per-message
    // boundaries are conveyed by repeated `message_start`; the whole turn ends
    // with an explicit `done` from the caller.
}

/// A PermissionUI that always denies (headless mode).
pub struct DenyAllPermissionUI;

#[async_trait::async_trait]
impl PermissionUI for DenyAllPermissionUI {
    async fn request(
        &self,
        _tool_name: &str,
        _input: &serde_json::Value,
    ) -> Option<PermissionDecision> {
        Some(PermissionDecision::Deny)
    }
}

/// A UserQuestionUI that always returns None (headless mode).
pub struct NoopUserQuestionUI;

#[async_trait::async_trait]
impl UserQuestionUI for NoopUserQuestionUI {
    async fn ask(&self, _request: AskUserQuestionRequest) -> Option<AskUserQuestionResponse> {
        None
    }
}

/// Trait for incremental session persistence from the agent loop.
///
/// The agent loop calls these methods at the appropriate points during execution.
/// Implementations handle the actual I/O (e.g., appending to a JSONL file).
pub trait SessionPersistence: Send + Sync {
    /// Persist a message (user or assistant) to the session log.
    fn persist_message(
        &mut self,
        msg: &Message,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Persist a session event (compaction boundary, permission change, etc.).
    fn persist_event(
        &mut self,
        event: &SessionEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

#[cfg(test)]
mod stream_json_tests {
    //! Tests for the stream-json NDJSON output sink. These guard the iteration
    //! 46 acceptance criteria: each emitted line is independently parseable by
    //! serde_json, event types appear in a stable order, and tool_use/result
    //! stay correlated by id.
    use super::*;

    /// Shareable byte buffer so a test can read what the sink wrote.
    struct SharedBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn capture() -> (StreamJsonOutputSink, Arc<Mutex<Vec<u8>>>) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let sink = StreamJsonOutputSink::new("sess-1", Box::new(SharedBuffer(buffer.clone())));
        (sink, buffer)
    }

    /// Drive the sink through a representative turn and return the parsed
    /// JSON values, one per line.
    fn parsed_events(buffer: &Arc<Mutex<Vec<u8>>>) -> Vec<serde_json::Value> {
        let captured = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        captured
            .trim_end()
            .lines()
            .map(|line| serde_json::from_str(line).expect("each line is valid JSON"))
            .collect()
    }

    #[test]
    fn emits_valid_ndjson_in_stable_order() {
        let (sink, buffer) = capture();
        sink.stream_start();
        sink.stream_delta("hello ");
        sink.stream_delta("world");
        sink.thinking_delta("reasoning");
        sink.tool_use_with_id("tu_1", "FileRead", &serde_json::json!({"path": "/tmp/a"}));
        sink.tool_result_with_id("tu_1", "FileRead", "contents", false);
        sink.usage(10, 20, 0, 0);
        sink.emit_done();

        let events = parsed_events(&buffer);
        let types: Vec<&str> = events
            .iter()
            .map(|v| v["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            vec![
                "message_start",
                "content_block_delta",
                "content_block_delta",
                "thinking_delta",
                "tool_use",
                "tool_result",
                "usage",
                "done",
            ]
        );

        // Spot-check the fields that carry correlation/identity.
        assert_eq!(events[0]["session_id"], "sess-1");
        assert_eq!(events[1]["delta"]["type"], "text_delta");
        assert_eq!(events[1]["delta"]["text"], "hello ");
        assert_eq!(events[4]["id"], "tu_1");
        assert_eq!(events[4]["name"], "FileRead");
        assert_eq!(events[4]["input"]["path"], "/tmp/a");
        assert_eq!(events[5]["tool_use_id"], "tu_1");
        assert_eq!(events[5]["name"], "FileRead");
        assert_eq!(events[5]["is_error"], false);
        assert_eq!(events[5]["content"], "contents");
        assert_eq!(events[6]["input_tokens"], 10);
        assert_eq!(events[6]["output_tokens"], 20);
    }

    #[test]
    fn each_line_is_independently_parseable() {
        // A multi-line tool result content must not break NDJSON framing:
        // serde_json::to_string escapes newlines, so one event == one line.
        let (sink, buffer) = capture();
        sink.stream_start();
        sink.tool_result_with_id("tu_1", "Bash", "line1\nline2\nline3", false);
        sink.emit_done();

        let captured = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
        let line_count = captured.trim_end().lines().count();
        assert_eq!(line_count, 3, "newline in content must not add lines");
        // Every line parses on its own.
        for line in captured.trim_end().lines() {
            serde_json::from_str::<serde_json::Value>(line).unwrap();
        }
    }

    #[test]
    fn multiple_tools_keep_call_result_order() {
        let (sink, buffer) = capture();
        sink.stream_start();
        sink.tool_use_with_id("tu_a", "FileRead", &serde_json::json!({"path": "/a"}));
        sink.tool_use_with_id("tu_b", "FileRead", &serde_json::json!({"path": "/b"}));
        sink.tool_result_with_id("tu_a", "FileRead", "A", false);
        sink.tool_result_with_id("tu_b", "FileRead", "B", false);
        sink.emit_done();

        let events = parsed_events(&buffer);
        let types: Vec<&str> = events.iter().map(|v| v["type"].as_str().unwrap()).collect();
        assert_eq!(
            types,
            vec![
                "message_start",
                "tool_use",
                "tool_use",
                "tool_result",
                "tool_result",
                "done",
            ]
        );
        // ids stay correlated even with two interleaved calls.
        assert_eq!(events[3]["tool_use_id"], "tu_a");
        assert_eq!(events[3]["content"], "A");
        assert_eq!(events[4]["tool_use_id"], "tu_b");
        assert_eq!(events[4]["content"], "B");
    }

    #[test]
    fn emit_error_then_done_terminates_the_stream() {
        let (sink, buffer) = capture();
        sink.stream_start();
        sink.emit_error("boom");
        sink.emit_done();

        let events = parsed_events(&buffer);
        let types: Vec<&str> = events.iter().map(|v| v["type"].as_str().unwrap()).collect();
        assert_eq!(types, vec!["message_start", "error", "done"]);
        assert_eq!(events[1]["message"], "boom");
    }

    #[test]
    fn sink_error_event_uses_same_shape_as_emit_error() {
        // Mid-run output.error() (e.g. compaction failures) must surface as a
        // parseable error event, same shape as terminal errors.
        let (sink, buffer) = capture();
        sink.stream_start();
        sink.error("compaction failed");
        sink.emit_done();

        let events = parsed_events(&buffer);
        assert_eq!(events[1]["type"], "error");
        assert_eq!(events[1]["message"], "compaction failed");
    }

    #[test]
    fn clones_share_one_writer() {
        // The CLI keeps a clone to emit the terminal done while handing another
        // clone to the query loop; both must write to the same stream.
        let (sink, buffer) = capture();
        let query_loop_clone = sink.clone();
        query_loop_clone.stream_start();
        query_loop_clone.stream_delta("hi");
        sink.emit_done();

        let events = parsed_events(&buffer);
        let types: Vec<&str> = events.iter().map(|v| v["type"].as_str().unwrap()).collect();
        assert_eq!(types, vec!["message_start", "content_block_delta", "done"]);
    }
}
