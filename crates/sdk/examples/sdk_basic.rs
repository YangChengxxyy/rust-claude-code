use rust_claude_sdk::Session;

#[tokio::main]
async fn main() -> rust_claude_sdk::Result<()> {
    let api_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| "demo-key".to_string());
    let session = Session::builder().api_key(api_key).build()?;
    let message = session.send("Hello from the Rust SDK").await?;

    for block in message.content {
        if let rust_claude_core::message::ContentBlock::Text { text } = block {
            println!("{text}");
        }
    }

    Ok(())
}
