pub mod hooks;
pub mod session;

// Re-exported from sdk crate for backward compatibility
pub use rust_claude_sdk::compaction;
pub use rust_claude_sdk::system_prompt;
pub mod query_loop {
    pub use rust_claude_sdk::agent_loop::*;
}
