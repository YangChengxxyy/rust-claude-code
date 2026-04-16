use crate::{message::Usage, permission::PermissionMode};
use std::borrow::Cow;

const DEFAULT_OPUS_MODEL: &str = "claude-opus-4-6";
const DEFAULT_SONNET_MODEL: &str = "claude-sonnet-4-6";
const DEFAULT_HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";
const TOKENS_200K_THRESHOLD: u32 = 200_000;

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
        return trimmed.to_string();
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
}
