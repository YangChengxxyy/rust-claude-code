use std::sync::{Arc, Mutex as StdMutex};
use std::sync::atomic::{AtomicBool, Ordering};

use rust_claude_core::state::AppState;
use rust_claude_core::tool_types::ToolResult;
use rust_claude_tools::{
    AgentContext, InterruptBehavior, ToolContext, ToolError, ToolRegistry, UserQuestionCallback,
};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct StreamingToolOutput {
    pub sequence: usize,
    pub name: String,
    pub input: serde_json::Value,
    pub run_post_hook: bool,
    pub result: ToolResult,
}

pub struct StreamingToolExecutor {
    tools: Arc<ToolRegistry>,
    app_state: Arc<Mutex<AppState>>,
    agent_context: Option<AgentContext>,
    user_question_callback: Option<UserQuestionCallback>,
    serial_lock: Arc<Mutex<()>>,
    cancellation_token: CancellationToken,
    bash_cancelled: Arc<AtomicBool>,
    tasks: StdMutex<Vec<PendingTask>>,
}

struct PendingTask {
    sequence: usize,
    name: String,
    input: serde_json::Value,
    interrupt_behavior: InterruptBehavior,
    handle: PendingHandle,
}

enum PendingHandle {
    Running(JoinHandle<Result<ToolResult, ToolError>>),
    Ready(ToolResult),
}

impl StreamingToolExecutor {
    pub fn new(
        tools: Arc<ToolRegistry>,
        app_state: Arc<Mutex<AppState>>,
        agent_context: Option<AgentContext>,
        user_question_callback: Option<UserQuestionCallback>,
    ) -> Self {
        Self {
            tools,
            app_state,
            agent_context,
            user_question_callback,
            serial_lock: Arc::new(Mutex::new(())),
            cancellation_token: CancellationToken::new(),
            bash_cancelled: Arc::new(AtomicBool::new(false)),
            tasks: StdMutex::new(Vec::new()),
        }
    }

    pub async fn add_tool(
        &self,
        sequence: usize,
        tool_use_id: &str,
        name: &str,
        input: serde_json::Value,
    ) -> Result<(), ToolError> {
        let tools = self.tools.clone();
        let app_state = self.app_state.clone();
        let agent_context = self.agent_context.clone();
        let user_question_callback = self.user_question_callback.clone();
        let serial_lock = self.serial_lock.clone();
        let cancellation_token = self.cancellation_token.clone();
        let bash_cancelled = self.bash_cancelled.clone();
        let is_concurrency_safe = self.tools.is_concurrency_safe(name);
        let interrupt_behavior = self
            .tools
            .get(name)
            .map(|tool| tool.interrupt_behavior)
            .unwrap_or(InterruptBehavior::Cancel);
        let tool_use_id = tool_use_id.to_string();
        let name = name.to_string();
        let task_name = name.clone();
        let task_input = input.clone();
        let handle = tokio::spawn(async move {
            if cancellation_token.is_cancelled() {
                return Ok(ToolResult::error(tool_use_id, "Tool execution cancelled".to_string()));
            }
            if name == "Bash" && bash_cancelled.load(Ordering::SeqCst) {
                return Ok(ToolResult::error(
                    tool_use_id,
                    "Bash execution cancelled after sibling failure".to_string(),
                ));
            }
            let _serial_guard = if is_concurrency_safe {
                None
            } else {
                Some(serial_lock.lock().await)
            };
            if name == "Bash" && bash_cancelled.load(Ordering::SeqCst) {
                return Ok(ToolResult::error(
                    tool_use_id,
                    "Bash execution cancelled after sibling failure".to_string(),
                ));
            }
            let result = tools
                .execute(
                    &name,
                    input,
                    ToolContext {
                        tool_use_id: tool_use_id.clone(),
                        app_state: Some(app_state),
                        agent_context,
                        user_question_callback,
                    },
                )
                .await;
            match result {
                Ok(result) => Ok(result),
                Err(error) => {
                    if name == "Bash" {
                        bash_cancelled.store(true, Ordering::SeqCst);
                    }
                    Ok(ToolResult::error(tool_use_id, error.to_string()))
                }
            }
        });

        self.tasks.lock().unwrap().push(PendingTask {
            sequence,
            name: task_name,
            input: task_input,
            interrupt_behavior,
            handle: PendingHandle::Running(handle),
        });
        Ok(())
    }

    pub async fn add_precomputed_result(
        &self,
        sequence: usize,
        name: &str,
        input: serde_json::Value,
        result: ToolResult,
    ) {
        self.tasks.lock().unwrap().push(PendingTask {
            sequence,
            name: name.to_string(),
            input,
            interrupt_behavior: InterruptBehavior::Cancel,
            handle: PendingHandle::Ready(result),
        });
    }

    pub async fn discard(self) -> Result<(), ToolError> {
        self.cancellation_token.cancel();
        let tasks = std::mem::take(&mut *self.tasks.lock().unwrap());
        for task in tasks {
            match task.interrupt_behavior {
                InterruptBehavior::Cancel => {
                    if let PendingHandle::Running(handle) = task.handle {
                        handle.abort();
                    }
                }
                InterruptBehavior::Block => {
                    if let PendingHandle::Running(handle) = task.handle {
                        handle
                            .await
                            .map_err(|error| ToolError::Execution(error.to_string()))??;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn finish(self) -> Result<Vec<StreamingToolOutput>, ToolError> {
        let mut completed = Vec::new();
        let tasks = std::mem::take(&mut *self.tasks.lock().unwrap());
        for task in tasks {
            let (run_post_hook, result) = match task.handle {
                PendingHandle::Running(handle) => (true, handle
                    .await
                    .map_err(|error| ToolError::Execution(error.to_string()))??),
                PendingHandle::Ready(result) => (false, result),
            };
            completed.push(StreamingToolOutput {
                sequence: task.sequence,
                name: task.name,
                input: task.input,
                run_post_hook,
                result,
            });
        }
        completed.sort_by_key(|output| output.sequence);
        Ok(completed)
    }
}

impl Drop for StreamingToolExecutor {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
        if let Ok(mut tasks) = self.tasks.lock() {
            for task in tasks.drain(..) {
                if task.interrupt_behavior == InterruptBehavior::Cancel {
                    if let PendingHandle::Running(handle) = task.handle {
                        handle.abort();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use async_trait::async_trait;
    use rust_claude_core::tool_types::ToolInfo;
    use rust_claude_tools::Tool;
    use tokio::sync::Notify;

    #[derive(Clone)]
    struct RecordingTool {
        name: &'static str,
        content: &'static str,
        delay: Duration,
        concurrency_safe: bool,
        starts: Arc<Mutex<Vec<&'static str>>>,
        notify: Arc<Notify>,
    }

    #[derive(Clone)]
    struct BlockingTool {
        started: Arc<Notify>,
        finished: Arc<AtomicBool>,
    }

    #[async_trait]
    impl Tool for BlockingTool {
        fn info(&self) -> ToolInfo {
            ToolInfo {
                name: "BlockingTool".to_string(),
                description: "Blocking test tool".to_string(),
                input_schema: serde_json::json!({}),
            }
        }

        fn is_concurrency_safe(&self) -> bool {
            true
        }

        fn interrupt_behavior(&self) -> InterruptBehavior {
            InterruptBehavior::Block
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            context: ToolContext,
        ) -> Result<ToolResult, ToolError> {
            self.started.notify_waiters();
            tokio::time::sleep(Duration::from_millis(20)).await;
            self.finished.store(true, Ordering::SeqCst);
            Ok(ToolResult::success(context.tool_use_id, "blocked".to_string()))
        }
    }

    #[async_trait]
    impl Tool for RecordingTool {
        fn info(&self) -> ToolInfo {
            ToolInfo {
                name: self.name.to_string(),
                description: format!("{} test tool", self.name),
                input_schema: serde_json::json!({}),
            }
        }

        fn is_concurrency_safe(&self) -> bool {
            self.concurrency_safe
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            context: ToolContext,
        ) -> Result<ToolResult, ToolError> {
            self.starts.lock().await.push(self.name);
            self.notify.notify_waiters();
            tokio::time::sleep(self.delay).await;
            Ok(ToolResult::success(
                context.tool_use_id,
                self.content.to_string(),
            ))
        }
    }

    #[tokio::test]
    async fn starts_concurrency_safe_tool_before_finish() {
        let starts = Arc::new(Mutex::new(Vec::new()));
        let notify = Arc::new(Notify::new());
        let mut registry = ToolRegistry::new();
        registry.register(RecordingTool {
            name: "FastRead",
            content: "read",
            delay: Duration::from_millis(50),
            concurrency_safe: true,
            starts: starts.clone(),
            notify: notify.clone(),
        });
        let executor = StreamingToolExecutor::new(
            Arc::new(registry),
            Arc::new(Mutex::new(AppState::new(std::env::current_dir().unwrap()))),
            None,
            None,
        );

        executor
            .add_tool(0, "tool_1", "FastRead", serde_json::json!({}))
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_millis(25), notify.notified())
            .await
            .expect("tool should start before finish is called");

        let results = executor.finish().await.unwrap();
        assert_eq!(results[0].result.tool_use_id, "tool_1");
        assert_eq!(results[0].result.content, "read");
    }

    #[tokio::test]
    async fn returns_results_in_tool_use_order() {
        let starts = Arc::new(Mutex::new(Vec::new()));
        let notify = Arc::new(Notify::new());
        let mut registry = ToolRegistry::new();
        registry.register(RecordingTool {
            name: "SlowRead",
            content: "slow",
            delay: Duration::from_millis(40),
            concurrency_safe: true,
            starts: starts.clone(),
            notify: notify.clone(),
        });
        registry.register(RecordingTool {
            name: "FastRead",
            content: "fast",
            delay: Duration::from_millis(1),
            concurrency_safe: true,
            starts,
            notify,
        });
        let executor = StreamingToolExecutor::new(
            Arc::new(registry),
            Arc::new(Mutex::new(AppState::new(std::env::current_dir().unwrap()))),
            None,
            None,
        );

        executor
            .add_tool(0, "tool_slow", "SlowRead", serde_json::json!({}))
            .await
            .unwrap();
        executor
            .add_tool(1, "tool_fast", "FastRead", serde_json::json!({}))
            .await
            .unwrap();

        let results = executor.finish().await.unwrap();
        assert_eq!(results[0].result.tool_use_id, "tool_slow");
        assert_eq!(results[0].result.content, "slow");
        assert_eq!(results[1].result.tool_use_id, "tool_fast");
        assert_eq!(results[1].result.content, "fast");
    }

    #[tokio::test]
    async fn serializes_non_concurrency_safe_tools() {
        let starts = Arc::new(Mutex::new(Vec::new()));
        let notify = Arc::new(Notify::new());
        let mut registry = ToolRegistry::new();
        registry.register(RecordingTool {
            name: "SlowWrite",
            content: "first",
            delay: Duration::from_millis(50),
            concurrency_safe: false,
            starts: starts.clone(),
            notify: notify.clone(),
        });
        registry.register(RecordingTool {
            name: "SecondWrite",
            content: "second",
            delay: Duration::from_millis(1),
            concurrency_safe: false,
            starts: starts.clone(),
            notify: notify.clone(),
        });
        let executor = StreamingToolExecutor::new(
            Arc::new(registry),
            Arc::new(Mutex::new(AppState::new(std::env::current_dir().unwrap()))),
            None,
            None,
        );

        executor
            .add_tool(0, "tool_first", "SlowWrite", serde_json::json!({}))
            .await
            .unwrap();
        executor
            .add_tool(1, "tool_second", "SecondWrite", serde_json::json!({}))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(*starts.lock().await, vec!["SlowWrite"]);

        let results = executor.finish().await.unwrap();
        assert_eq!(*starts.lock().await, vec!["SlowWrite", "SecondWrite"]);
        assert_eq!(results[0].result.content, "first");
        assert_eq!(results[1].result.content, "second");
    }

    #[tokio::test]
    async fn discard_waits_for_blocking_tools() {
        let started = Arc::new(Notify::new());
        let finished = Arc::new(AtomicBool::new(false));
        let mut registry = ToolRegistry::new();
        registry.register(BlockingTool {
            started: started.clone(),
            finished: finished.clone(),
        });
        let executor = StreamingToolExecutor::new(
            Arc::new(registry),
            Arc::new(Mutex::new(AppState::new(std::env::current_dir().unwrap()))),
            None,
            None,
        );

        executor
            .add_tool(0, "tool_block", "BlockingTool", serde_json::json!({}))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_millis(10), started.notified())
            .await
            .unwrap();

        executor.discard().await.unwrap();

        assert!(
            finished.load(Ordering::SeqCst),
            "discard should wait for blocking tool to finish"
        );
    }

    #[derive(Clone)]
    struct FailingTool {
        name: &'static str,
        starts: Arc<Mutex<Vec<&'static str>>>,
        notify: Arc<Notify>,
        delay: Duration,
    }

    #[async_trait]
    impl Tool for FailingTool {
        fn info(&self) -> ToolInfo {
            ToolInfo {
                name: self.name.to_string(),
                description: "Failing test tool".to_string(),
                input_schema: serde_json::json!({}),
            }
        }

        fn is_concurrency_safe(&self) -> bool {
            self.name != "Bash"
        }

        async fn execute(
            &self,
            _input: serde_json::Value,
            _context: ToolContext,
        ) -> Result<ToolResult, ToolError> {
            self.starts.lock().await.push(self.name);
            self.notify.notify_waiters();
            tokio::time::sleep(self.delay).await;
            Err(ToolError::Execution("failed".to_string()))
        }
    }

    #[tokio::test]
    async fn bash_failure_cancels_later_bash_but_not_file_read() {
        let starts = Arc::new(Mutex::new(Vec::new()));
        let notify = Arc::new(Notify::new());
        let mut registry = ToolRegistry::new();
        registry.register(FailingTool {
            name: "Bash",
            starts: starts.clone(),
            notify: notify.clone(),
            delay: Duration::from_millis(0),
        });
        registry.register(RecordingTool {
            name: "FileRead",
            content: "read",
            delay: Duration::from_millis(1),
            concurrency_safe: true,
            starts: starts.clone(),
            notify: notify.clone(),
        });
        let executor = StreamingToolExecutor::new(
            Arc::new(registry),
            Arc::new(Mutex::new(AppState::new(std::env::current_dir().unwrap()))),
            None,
            None,
        );

        executor
            .add_tool(0, "tool_fail", "Bash", serde_json::json!({}))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_millis(10), notify.notified())
            .await
            .unwrap();
        executor
            .add_tool(1, "tool_read", "FileRead", serde_json::json!({}))
            .await
            .unwrap();
        executor
            .add_tool(2, "tool_late_bash", "Bash", serde_json::json!({}))
            .await
            .unwrap();

        let results = executor.finish().await.unwrap();
        assert_eq!(*starts.lock().await, vec!["Bash", "FileRead"]);
        assert!(results[0].result.is_error);
        assert_eq!(results[1].result.content, "read");
        assert!(results[2].result.is_error);
    }

    #[tokio::test]
    async fn bash_failure_cancels_bash_already_waiting_on_serial_lock() {
        let starts = Arc::new(Mutex::new(Vec::new()));
        let notify = Arc::new(Notify::new());
        let mut registry = ToolRegistry::new();
        registry.register(FailingTool {
            name: "Bash",
            starts: starts.clone(),
            notify: notify.clone(),
            delay: Duration::from_millis(20),
        });
        registry.register(RecordingTool {
            name: "SlowWrite",
            content: "write",
            delay: Duration::from_millis(40),
            concurrency_safe: false,
            starts: starts.clone(),
            notify: notify.clone(),
        });
        let executor = StreamingToolExecutor::new(
            Arc::new(registry),
            Arc::new(Mutex::new(AppState::new(std::env::current_dir().unwrap()))),
            None,
            None,
        );

        executor
            .add_tool(0, "tool_write", "SlowWrite", serde_json::json!({}))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_millis(10), notify.notified())
            .await
            .unwrap();
        executor
            .add_tool(1, "tool_fail", "Bash", serde_json::json!({}))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        executor
            .add_tool(2, "tool_waiting_bash", "Bash", serde_json::json!({}))
            .await
            .unwrap();

        let results = executor.finish().await.unwrap();
        assert_eq!(*starts.lock().await, vec!["SlowWrite", "Bash"]);
        assert!(!results[0].result.is_error);
        assert!(results[1].result.is_error);
        assert!(results[2].result.is_error);
    }
}
