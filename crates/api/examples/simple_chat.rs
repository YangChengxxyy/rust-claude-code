use std::io::{self, Write};

use rust_claude_api::{AnthropicClient, ApiMessage, CreateMessageRequest};
use rust_claude_core::message::ContentBlock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")?;
    let client = AnthropicClient::new(api_key)?;

    print!("You: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let request = CreateMessageRequest::new(
        "claude-sonnet-4-20250514",
        vec![ApiMessage::user(input.trim_end())],
    )
    .with_max_tokens(1024);

    let response = client.create_message(&request).await?;
    println!("Claude: {}", format_blocks(&response.content));

    Ok(())
}

fn format_blocks(blocks: &[ContentBlock]) -> String {
    blocks
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.clone(),
            ContentBlock::ToolUse { name, input, .. } => {
                format!("[tool_use:{} {}]", name, input)
            }
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                let status = if *is_error { "error" } else { "ok" };
                format!("[tool_result:{}:{} {}]", tool_use_id, status, content)
            }
            ContentBlock::Thinking { thinking, .. } => format!("[thinking {}]", thinking),
            ContentBlock::Image { .. } => "[image block]".to_string(),
            ContentBlock::Unknown => "[unknown block]".to_string(),
        })
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::format_blocks;
    use rust_claude_core::message::ContentBlock;

    #[test]
    fn format_blocks_includes_non_text_blocks() {
        let output = format_blocks(&[
            ContentBlock::thinking("plan first"),
            ContentBlock::tool_use("tool_1", "Bash", serde_json::json!({ "command": "pwd" })),
        ]);

        assert!(output.contains("[thinking plan first]"));
        assert!(output.contains("[tool_use:Bash"));
    }
}
