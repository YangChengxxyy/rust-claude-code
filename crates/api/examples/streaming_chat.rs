use futures_util::StreamExt;
use rust_claude_api::{
    AnthropicClient, ApiMessage, ContentBlockDelta, CreateMessageRequest, StreamEvent,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")?;
    let client = AnthropicClient::new(api_key)?;

    let request = CreateMessageRequest::new(
        "claude-sonnet-4-20250514",
        vec![ApiMessage::user("Write a one-sentence greeting.")],
    )
    .with_max_tokens(128)
    .with_stream(true);

    let mut stream = client.create_message_stream(&request).await?;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::ContentBlockDelta {
                delta: ContentBlockDelta::TextDelta { text },
                ..
            } => {
                print!("{text}");
            }
            StreamEvent::MessageStop => {
                println!();
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
