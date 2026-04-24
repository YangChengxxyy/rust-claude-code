use crate::{message::Usage, permission::PermissionMode};
use std::borrow::Cow;

const DEFAULT_OPUS_MODEL: &str = "claude-opus-4-6";
const DEFAULT_SONNET_MODEL: &str = "claude-sonnet-4-6";
const DEFAULT_HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";
const TOKENS_200K_THRESHOLD: u32 = 200_000;
const DEFAULT_THINKING_BUDGET: u32 = 10_000;

/// Configuration for extended thinking in API requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThinkingConfig {
    /// Thinking is disabled; `thinking` field omitted from API request.
    Disabled,
    /// Budget-based thinking with explicit token budget.
    Enabled { budget_tokens: u32 },
    /// Adaptive thinking (supported by Opus 4.6 / Sonnet 4.6).
    Adaptive,
}

impl ThinkingConfig {
    /// Serialize to the Anthropic API format for the `thinking` field.
    /// Returns `None` if disabled (field should be omitted).
    pub fn to_api_value(&self, max_tokens: u32) -> Option<serde_json::Value> {
        match self {
            Self::Disabled => None,
            Self::Enabled { budget_tokens } => {
                // API constraint: max_tokens > budget_tokens
                let budget = (*budget_tokens).min(max_tokens.saturating_sub(1));
                Some(serde_json::json!({
                    "type": "enabled",
                    "budget_tokens": budget
                }))
            }
            Self::Adaptive => Some(serde_json::json!({
                "type": "enabled",
                "budget_tokens": max_tokens.saturating_sub(1)
            })),
        }
    }
}

/// Check if a model string represents a model that supports extended thinking.
pub fn model_supports_thinking(model: &str) -> bool {
    let normalized = normalize_model_string_for_api(model).to_lowercase();
    // Claude 4+ models support thinking
    normalized.contains("claude-opus-4")
        || normalized.contains("claude-sonnet-4")
        || normalized.contains("claude-haiku-4")
}

/// Check if a model supports adaptive thinking (no explicit budget needed).
pub fn model_supports_adaptive_thinking(model: &str) -> bool {
    let normalized = normalize_model_string_for_api(model).to_lowercase();
    // Only Opus 4.6 and Sonnet 4.6 support adaptive thinking
    normalized.contains("opus-4-6") || normalized.contains("sonnet-4-6")
}

/// Determine the appropriate ThinkingConfig for a given model.
pub fn get_thinking_config_for_model(model: &str, thinking_enabled: bool) -> ThinkingConfig {
    if !thinking_enabled {
        return ThinkingConfig::Disabled;
    }

    if !model_supports_thinking(model) {
        return ThinkingConfig::Disabled;
    }

    if model_supports_adaptive_thinking(model) {
        ThinkingConfig::Adaptive
    } else {
        ThinkingConfig::Enabled {
            budget_tokens: DEFAULT_THINKING_BUDGET,
        }
    }
}

pub fn get_default_opus_model() -> &'static str {
    DEFAULT_OPUS_MODEL
}

pub fn get_default_sonnet_model() -> &'static str {
    DEFAULT_SONNET_MODEL
}

pub fn get_default_haiku_model() -> &'static str {
    DEFAULT_HAIKU_MODEL
}

pub fn get_best_model() -> &'static str {
    get_default_opus_model()
}

pub fn parse_user_specified_model(model_input: &str) -> String {
    let trimmed = model_input.trim();
    if trimmed.is_empty() {
        return get_default_opus_model().to_string();
    }

    let lower = trimmed.to_ascii_lowercase();
    let has_1m_tag = has_suffix_ignore_ascii_case(trimmed, "[1m]");
    let base = if has_1m_tag {
        lower.trim_end_matches("[1m]").trim()
    } else {
        lower.as_str()
    };

    let resolved = match base {
        "opus" => Cow::Borrowed(get_default_opus_model()),
        "sonnet" => Cow::Borrowed(get_default_sonnet_model()),
        "haiku" => Cow::Borrowed(get_default_haiku_model()),
        "best" => Cow::Borrowed(get_best_model()),
        "opusplan" => Cow::Borrowed(get_default_sonnet_model()),
        _ => Cow::Borrowed(trimmed),
    };

    if has_1m_tag {
        format!("{}[1m]", trim_suffix_ignore_ascii_case(resolved.as_ref(), "[1m]"))
    } else {
        resolved.into_owned()
    }
}

pub fn usage_exceeds_200k_tokens(usage: &Usage) -> bool {
    let total = usage.input_tokens
        + usage.output_tokens
        + usage.cache_creation_input_tokens
        + usage.cache_read_input_tokens;
    total > TOKENS_200K_THRESHOLD
}

pub fn get_runtime_main_loop_model(
    model_setting: &str,
    permission_mode: PermissionMode,
    exceeds_200k_tokens: bool,
) -> String {
    if model_setting.eq_ignore_ascii_case("opusplan")
        && permission_mode == PermissionMode::Plan
        && !exceeds_200k_tokens
    {
        return get_default_opus_model().to_string();
    }

    if model_setting.eq_ignore_ascii_case("haiku") && permission_mode == PermissionMode::Plan {
        return get_default_sonnet_model().to_string();
    }

    parse_user_specified_model(model_setting)
}

pub fn normalize_model_string_for_api(model: &str) -> String {
    let normalized = trim_suffix_ignore_ascii_case(model.trim(), "[1m]");
    let normalized = trim_suffix_ignore_ascii_case(normalized, "[2m]");
    normalized.to_string()
}

fn has_suffix_ignore_ascii_case(value: &str, suffix: &str) -> bool {
    value
        .get(value.len().saturating_sub(suffix.len())..)
        .is_some_and(|tail| tail.eq_ignore_ascii_case(suffix))
}

fn trim_suffix_ignore_ascii_case<'a>(value: &'a str, suffix: &str) -> &'a str {
    if has_suffix_ignore_ascii_case(value, suffix) {
        &value[..value.len() - suffix.len()]
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_opus_alias_with_1m_suffix() {
        assert_eq!(parse_user_specified_model("opus[1m]"), "claude-opus-4-6[1m]");
    }

    #[test]
    fn resolves_sonnet_alias() {
        assert_eq!(parse_user_specified_model("sonnet"), "claude-sonnet-4-6");
    }

    #[test]
    fn resolves_haiku_alias() {
        assert_eq!(parse_user_specified_model("haiku"), "claude-haiku-4-5-20251001");
    }

    #[test]
    fn resolves_best_alias_to_opus() {
        assert_eq!(parse_user_specified_model("best"), "claude-opus-4-6");
    }

    #[test]
    fn resolves_opusplan_to_sonnet_outside_plan_mode() {
        assert_eq!(parse_user_specified_model("opusplan"), "claude-sonnet-4-6");
    }

    #[test]
    fn usage_above_200k_threshold_returns_true() {
        let usage = Usage {
            input_tokens: 150_000,
            output_tokens: 40_000,
            cache_creation_input_tokens: 20_000,
            cache_read_input_tokens: 1,
        };
        assert!(usage_exceeds_200k_tokens(&usage));
    }

    #[test]
    fn usage_at_200k_threshold_returns_false() {
        let usage = Usage {
            input_tokens: 150_000,
            output_tokens: 40_000,
            cache_creation_input_tokens: 10_000,
            cache_read_input_tokens: 0,
        };
        assert!(!usage_exceeds_200k_tokens(&usage));
    }

    #[test]
    fn opusplan_switches_to_opus_in_plan_mode_under_200k() {
        assert_eq!(
            get_runtime_main_loop_model("opusplan", PermissionMode::Plan, false),
            "claude-opus-4-6"
        );
    }

    #[test]
    fn opusplan_stays_sonnet_when_over_200k() {
        assert_eq!(
            get_runtime_main_loop_model("opusplan", PermissionMode::Plan, true),
            "claude-sonnet-4-6"
        );
    }

    #[test]
    fn haiku_switches_to_sonnet_in_plan_mode() {
        assert_eq!(
            get_runtime_main_loop_model("haiku", PermissionMode::Plan, false),
            "claude-sonnet-4-6"
        );
    }

    #[test]
    fn preserves_custom_model_case_without_suffix() {
        assert_eq!(
            parse_user_specified_model("MyCustomDeployment"),
            "MyCustomDeployment"
        );
    }

    #[test]
    fn preserves_custom_model_and_strips_suffix_case_insensitively() {
        assert_eq!(
            parse_user_specified_model("MyCustomDeployment[1M]"),
            "MyCustomDeployment[1m]"
        );
    }

    #[test]
    fn normalizes_1m_suffix_for_api() {
        assert_eq!(
            normalize_model_string_for_api("claude-opus-4-6[1m]"),
            "claude-opus-4-6"
        );
    }

    #[test]
    fn normalizes_2m_suffix_for_api_case_insensitively() {
        assert_eq!(
            normalize_model_string_for_api("claude-opus-4-6[2M]"),
            "claude-opus-4-6"
        );
    }

    // -- ThinkingConfig tests --

    #[test]
    fn thinking_config_disabled_returns_none() {
        assert_eq!(ThinkingConfig::Disabled.to_api_value(16384), None);
    }

    #[test]
    fn thinking_config_enabled_serializes_correctly() {
        let val = ThinkingConfig::Enabled { budget_tokens: 10000 }
            .to_api_value(16384)
            .unwrap();
        assert_eq!(val["type"], "enabled");
        assert_eq!(val["budget_tokens"], 10000);
    }

    #[test]
    fn thinking_config_enabled_caps_budget_to_max_tokens_minus_1() {
        let val = ThinkingConfig::Enabled { budget_tokens: 20000 }
            .to_api_value(16384)
            .unwrap();
        assert_eq!(val["budget_tokens"], 16383);
    }

    #[test]
    fn thinking_config_adaptive_uses_max_tokens() {
        let val = ThinkingConfig::Adaptive.to_api_value(16384).unwrap();
        assert_eq!(val["type"], "enabled");
        assert_eq!(val["budget_tokens"], 16383);
    }

    // -- Model thinking support tests --

    #[test]
    fn opus_4_6_supports_adaptive_thinking() {
        assert!(model_supports_thinking("claude-opus-4-6"));
        assert!(model_supports_adaptive_thinking("claude-opus-4-6"));
        assert!(model_supports_adaptive_thinking("claude-opus-4-6-20250514"));
        assert!(model_supports_adaptive_thinking("claude-opus-4-6[1m]"));
    }

    #[test]
    fn sonnet_4_6_supports_adaptive_thinking() {
        assert!(model_supports_thinking("claude-sonnet-4-6"));
        assert!(model_supports_adaptive_thinking("claude-sonnet-4-6-20250514"));
    }

    #[test]
    fn claude_3_5_does_not_support_thinking() {
        assert!(!model_supports_thinking("claude-3-5-sonnet-20241022"));
        assert!(!model_supports_adaptive_thinking("claude-3-5-sonnet-20241022"));
    }

    #[test]
    fn get_thinking_config_opus_4_6_returns_adaptive() {
        let config = get_thinking_config_for_model("claude-opus-4-6", true);
        assert_eq!(config, ThinkingConfig::Adaptive);
    }

    #[test]
    fn get_thinking_config_disabled_returns_disabled() {
        let config = get_thinking_config_for_model("claude-opus-4-6", false);
        assert_eq!(config, ThinkingConfig::Disabled);
    }

    #[test]
    fn get_thinking_config_claude_3_returns_disabled() {
        let config = get_thinking_config_for_model("claude-3-5-sonnet-20241022", true);
        assert_eq!(config, ThinkingConfig::Disabled);
    }

    #[test]
    fn get_thinking_config_haiku_4_returns_enabled() {
        let config = get_thinking_config_for_model("claude-haiku-4-5-20251001", true);
        assert!(matches!(config, ThinkingConfig::Enabled { .. }));
    }
}
