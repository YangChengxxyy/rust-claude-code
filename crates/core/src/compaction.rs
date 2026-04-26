use serde::{Deserialize, Serialize};

use crate::message::{ContentBlock, Message, Usage};

/// Configuration for conversation compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionConfig {
    /// Model context window size in tokens.
    pub context_window: u32,
    /// Trigger compaction when estimated tokens exceed this ratio of context_window.
    pub threshold_ratio: f32,
    /// Preserve recent messages up to this ratio of context_window.
    pub preserve_ratio: f32,
    /// Max tokens for the summary generation request.
    pub summary_max_tokens: u32,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            context_window: 200_000,
            threshold_ratio: 0.8,
            preserve_ratio: 0.5,
            summary_max_tokens: 8192,
        }
    }
}

/// Result of a compaction operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResult {
    pub original_message_count: usize,
    pub compacted_message_count: usize,
    pub preserved_message_count: usize,
    pub estimated_tokens_before: u32,
    pub estimated_tokens_after: u32,
    pub summary_length: usize,
}

/// Estimate the token count of a single content block using chars / 4 heuristic.
fn estimate_content_block_tokens(block: &ContentBlock) -> u32 {
    let char_count = match block {
        ContentBlock::Text { text } => text.chars().count(),
        ContentBlock::ToolUse { id, name, input } => {
            id.len() + name.len() + input.to_string().chars().count()
        }
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } => tool_use_id.len() + content.chars().count(),
        ContentBlock::Thinking { thinking, .. } => thinking.chars().count(),
        ContentBlock::Image { source } => match source {
            crate::message::ImageSource::Base64 { media_type, data } => {
                media_type.len() + data.len()
            }
            crate::message::ImageSource::Url { url } => url.len(),
        },
        ContentBlock::Unknown => 0,
    };
    (char_count as u32) / 4
}

/// Estimate the total token count for a list of messages.
pub fn estimate_message_tokens(messages: &[Message]) -> u32 {
    messages
        .iter()
        .map(|msg| {
            // Add a small overhead per message for role/structure tokens
            let block_tokens: u32 = msg.content.iter().map(estimate_content_block_tokens).sum();
            block_tokens + 4 // ~4 tokens overhead per message
        })
        .sum()
}

/// Estimate the token count of a system prompt.
pub fn estimate_system_prompt_tokens(system_prompt: &Option<String>) -> u32 {
    match system_prompt {
        Some(text) => (text.len() as u32) / 4,
        None => 0,
    }
}

/// Estimate the current conversation token count using a two-tier approach:
/// 1. If API usage data is available, use `input_tokens` from the last response
///    plus chars/4 estimate for any new messages added since.
/// 2. Fall back to full chars/4 heuristic for the first turn (no API data).
pub fn estimate_current_tokens(
    system_prompt: &Option<String>,
    messages: &[Message],
    last_api_usage: Option<&Usage>,
    last_api_message_index: usize,
) -> u32 {
    match last_api_usage {
        Some(usage) if last_api_message_index > 0 => {
            let base = usage.input_tokens;
            let new_messages = if last_api_message_index < messages.len() {
                &messages[last_api_message_index..]
            } else {
                &[]
            };
            base + estimate_message_tokens(new_messages)
        }
        _ => {
            // First turn fallback: full heuristic
            estimate_system_prompt_tokens(system_prompt) + estimate_message_tokens(messages)
        }
    }
}

/// Check whether compaction is needed based on the estimated token count.
pub fn needs_compaction(
    config: &CompactionConfig,
    system_prompt: &Option<String>,
    messages: &[Message],
    last_api_usage: Option<&Usage>,
    last_api_message_index: usize,
) -> bool {
    if messages.len() <= 4 {
        return false;
    }
    let total_tokens = estimate_current_tokens(
        system_prompt,
        messages,
        last_api_usage,
        last_api_message_index,
    );
    let threshold = (config.context_window as f32 * config.threshold_ratio) as u32;
    total_tokens > threshold
}

/// Partition messages into (to_compact, to_preserve).
///
/// Scans from the end, accumulating messages into the preserve set until
/// the preserve budget is exceeded. The rest go into the compact set.
/// Always preserves at least 2 messages. Returns `(empty, all)` if there
/// are too few messages to compact.
pub fn partition_messages(
    config: &CompactionConfig,
    messages: &[Message],
) -> (Vec<Message>, Vec<Message>) {
    if messages.len() <= 4 {
        return (Vec::new(), messages.to_vec());
    }

    let preserve_budget = (config.context_window as f32 * config.preserve_ratio) as u32;
    let mut preserve_tokens: u32 = 0;
    let mut split_index = messages.len(); // start from all-preserve

    for (i, msg) in messages.iter().enumerate().rev() {
        let msg_tokens: u32 = msg.content.iter().map(estimate_content_block_tokens).sum();
        let msg_tokens = msg_tokens + 4;
        if preserve_tokens + msg_tokens > preserve_budget {
            split_index = i + 1;
            break;
        }
        preserve_tokens += msg_tokens;
    }

    // Ensure at least 2 messages are preserved
    let min_preserve = 2;
    if messages.len() - split_index < min_preserve {
        split_index = messages.len().saturating_sub(min_preserve);
    }

    // Need at least 1 message to compact
    if split_index == 0 {
        return (Vec::new(), messages.to_vec());
    }

    let to_compact = messages[..split_index].to_vec();
    let to_preserve = messages[split_index..].to_vec();
    (to_compact, to_preserve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{ContentBlock, Message, Role};

    fn make_text_message(role: Role, text: &str) -> Message {
        Message {
            role,
            content: vec![ContentBlock::text(text)],
            usage: None,
        }
    }

    // -- CompactionConfig tests --

    #[test]
    fn test_compaction_config_default() {
        let config = CompactionConfig::default();
        assert_eq!(config.context_window, 200_000);
        assert!((config.threshold_ratio - 0.8).abs() < f32::EPSILON);
        assert!((config.preserve_ratio - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.summary_max_tokens, 8192);
    }

    #[test]
    fn test_compaction_config_serde() {
        let config = CompactionConfig {
            context_window: 100_000,
            threshold_ratio: 0.7,
            preserve_ratio: 0.4,
            summary_max_tokens: 4096,
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: CompactionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.context_window, 100_000);
        assert!((parsed.threshold_ratio - 0.7).abs() < f32::EPSILON);
    }

    // -- CompactionResult tests --

    #[test]
    fn test_compaction_result_serde() {
        let result = CompactionResult {
            original_message_count: 20,
            compacted_message_count: 12,
            preserved_message_count: 8,
            estimated_tokens_before: 170_000,
            estimated_tokens_after: 85_000,
            summary_length: 3000,
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: CompactionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.original_message_count, 20);
        assert_eq!(parsed.compacted_message_count, 12);
        assert_eq!(parsed.preserved_message_count, 8);
    }

    // -- Token estimation tests --

    #[test]
    fn test_estimate_message_tokens_text_only() {
        // 4000 chars of text -> ~1000 tokens + overhead
        let text = "a".repeat(4000);
        let messages = vec![make_text_message(Role::User, &text)];
        let estimate = estimate_message_tokens(&messages);
        // 4000/4 = 1000 + 4 overhead = 1004
        assert_eq!(estimate, 1004);
    }

    #[test]
    fn test_estimate_message_tokens_with_tool_use() {
        let messages = vec![Message {
            role: Role::Assistant,
            content: vec![ContentBlock::tool_use(
                "tool_123",
                "Bash",
                serde_json::json!({"command": "ls -la /tmp"}),
            )],
            usage: None,
        }];
        let estimate = estimate_message_tokens(&messages);
        assert!(estimate > 0);
    }

    #[test]
    fn test_estimate_message_tokens_with_tool_result() {
        let messages = vec![Message {
            role: Role::User,
            content: vec![ContentBlock::tool_result(
                "tool_123",
                "file1.txt\nfile2.txt\nfile3.txt",
                false,
            )],
            usage: None,
        }];
        let estimate = estimate_message_tokens(&messages);
        assert!(estimate > 0);
    }

    #[test]
    fn test_estimate_system_prompt_tokens() {
        let prompt = Some("a".repeat(2000));
        assert_eq!(estimate_system_prompt_tokens(&prompt), 500);
        assert_eq!(estimate_system_prompt_tokens(&None), 0);
    }

    // -- needs_compaction tests --

    #[test]
    fn test_needs_compaction_below_threshold() {
        let config = CompactionConfig {
            context_window: 200_000,
            ..Default::default()
        };
        // Small messages, well below threshold
        let messages: Vec<Message> = (0..6)
            .map(|i| {
                make_text_message(
                    if i % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    "hello world",
                )
            })
            .collect();
        assert!(!needs_compaction(&config, &None, &messages, None, 0));
    }

    #[test]
    fn test_needs_compaction_above_threshold() {
        let config = CompactionConfig {
            context_window: 1000, // small window for testing
            threshold_ratio: 0.8,
            ..Default::default()
        };
        // Each message: 400 chars -> ~100 tokens. 6 messages -> ~600+ tokens > 800 threshold? No.
        // Let's use bigger messages.
        let big_text = "x".repeat(2000); // 500 tokens each
        let messages: Vec<Message> = (0..6)
            .map(|i| {
                make_text_message(
                    if i % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    &big_text,
                )
            })
            .collect();
        // Total: ~6 * (500+4) = 3024 > 800 (1000 * 0.8)
        assert!(needs_compaction(&config, &None, &messages, None, 0));
    }

    #[test]
    fn test_needs_compaction_too_few_messages() {
        let config = CompactionConfig {
            context_window: 100, // tiny window
            ..Default::default()
        };
        let big_text = "x".repeat(2000);
        let messages = vec![
            make_text_message(Role::User, &big_text),
            make_text_message(Role::Assistant, &big_text),
        ];
        // Only 2 messages, should not compact even if above threshold
        assert!(!needs_compaction(&config, &None, &messages, None, 0));
    }

    // -- partition_messages tests --

    #[test]
    fn test_partition_too_few_messages() {
        let config = CompactionConfig::default();
        let messages = vec![
            make_text_message(Role::User, "hi"),
            make_text_message(Role::Assistant, "hello"),
        ];
        let (compact, preserve) = partition_messages(&config, &messages);
        assert!(compact.is_empty());
        assert_eq!(preserve.len(), 2);
    }

    #[test]
    fn test_partition_normal() {
        let config = CompactionConfig {
            context_window: 1000,
            preserve_ratio: 0.5, // preserve up to 500 tokens
            ..Default::default()
        };
        // Each message: 400 chars -> 100 tokens + 4 = 104
        let text = "a".repeat(400);
        let messages: Vec<Message> = (0..10)
            .map(|i| {
                make_text_message(
                    if i % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    &text,
                )
            })
            .collect();
        let (compact, preserve) = partition_messages(&config, &messages);
        // Budget: 500 tokens. Each msg ~104 tokens. Can fit ~4 messages.
        // So compact should have 6, preserve should have 4.
        assert!(!compact.is_empty());
        assert!(preserve.len() >= 2);
        assert_eq!(compact.len() + preserve.len(), 10);
    }

    #[test]
    fn test_partition_preserves_minimum_two() {
        let config = CompactionConfig {
            context_window: 100,
            preserve_ratio: 0.5, // preserve budget: 50 tokens
            ..Default::default()
        };
        // Each message huge: 1000 chars -> 250 tokens. Even 1 exceeds budget.
        let big_text = "x".repeat(1000);
        let messages: Vec<Message> = (0..6)
            .map(|i| {
                make_text_message(
                    if i % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    &big_text,
                )
            })
            .collect();
        let (compact, preserve) = partition_messages(&config, &messages);
        assert!(preserve.len() >= 2);
        assert_eq!(compact.len() + preserve.len(), 6);
    }

    #[test]
    fn test_partition_ordering() {
        let config = CompactionConfig {
            context_window: 1000,
            preserve_ratio: 0.5,
            ..Default::default()
        };
        let text = "a".repeat(400);
        let messages: Vec<Message> = (0..8)
            .map(|i| make_text_message(Role::User, &format!("msg{i}: {text}")))
            .collect();
        let (compact, preserve) = partition_messages(&config, &messages);
        // Verify ordering: compact messages are the earliest, preserve are the latest
        if !compact.is_empty() {
            if let ContentBlock::Text { text } = &compact[0].content[0] {
                assert!(text.starts_with("msg0:"));
            }
        }
        if !preserve.is_empty() {
            if let ContentBlock::Text { text } = &preserve.last().unwrap().content[0] {
                assert!(text.starts_with(&format!("msg{}:", messages.len() - 1)));
            }
        }
    }

    // -- estimate_current_tokens tests --

    #[test]
    fn test_estimate_current_tokens_no_api_usage() {
        // First turn: falls back to full chars/4
        let messages = vec![make_text_message(Role::User, &"a".repeat(4000))];
        let result = estimate_current_tokens(&None, &messages, None, 0);
        // 4000/4 + 4 overhead = 1004
        assert_eq!(result, 1004);
    }

    #[test]
    fn test_estimate_current_tokens_with_api_usage_no_new_messages() {
        let usage = Usage {
            input_tokens: 15000,
            output_tokens: 500,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let messages = vec![
            make_text_message(Role::User, "hello"),
            make_text_message(Role::Assistant, "hi"),
        ];
        // API was called after 2 messages, no new messages since
        let result = estimate_current_tokens(&None, &messages, Some(&usage), 2);
        assert_eq!(result, 15000);
    }

    #[test]
    fn test_estimate_current_tokens_with_api_usage_and_new_messages() {
        let usage = Usage {
            input_tokens: 15000,
            output_tokens: 500,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let messages = vec![
            make_text_message(Role::User, "hello"),
            make_text_message(Role::Assistant, "hi"),
            // New messages after API call:
            make_text_message(Role::User, &"x".repeat(2000)), // ~500 tokens + 4 overhead
        ];
        let result = estimate_current_tokens(&None, &messages, Some(&usage), 2);
        // 15000 + 504
        assert_eq!(result, 15504);
    }

    #[test]
    fn test_needs_compaction_with_usage_based_counting() {
        let config = CompactionConfig {
            context_window: 200_000,
            threshold_ratio: 0.8,
            ..Default::default()
        };
        let usage = Usage {
            input_tokens: 155_000,
            output_tokens: 500,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        // 6 messages, API says 155K tokens, new messages add ~7.5K
        let text = "a".repeat(10000); // ~2504 tokens per message
        let messages: Vec<Message> = (0..6)
            .map(|i| {
                make_text_message(
                    if i % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    &text,
                )
            })
            .collect();
        // With usage: 155000 + estimate(messages[4..6]) = 155000 + ~5008 = 160008 > 160000 threshold
        assert!(needs_compaction(&config, &None, &messages, Some(&usage), 4));
    }
}
