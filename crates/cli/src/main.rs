use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Parser;
use rust_claude_api::AnthropicClient;
use rust_claude_core::{
    claude_md,
    config::{Config, ConfigError},
    message::ContentBlock,
    model::get_runtime_main_loop_model,
    permission::PermissionMode,
    settings::ClaudeSettings,
    state::AppState,
};
use rust_claude_tools::{
    BashTool, FileEditTool, FileReadTool, FileWriteTool, GlobTool, GrepTool, TodoWriteTool,
    ToolRegistry,
};
use rust_claude_tui::{App, TerminalGuard, TuiBridge, UserCommand};
use tokio::sync::{mpsc, Mutex};

use rust_claude_cli::compaction::CompactionService;
use rust_claude_cli::query_loop::QueryLoop;
use rust_claude_cli::session::{self, SessionFile};
use rust_claude_cli::system_prompt;
use rust_claude_core::compaction::CompactionConfig;

// ---------------------------------------------------------------------------
// CLI argument definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ModeArg(PermissionMode);

impl std::str::FromStr for ModeArg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(ModeArg(PermissionMode::Default)),
            "accept-edits" => Ok(ModeArg(PermissionMode::AcceptEdits)),
            "bypass" => Ok(ModeArg(PermissionMode::BypassPermissions)),
            "plan" => Ok(ModeArg(PermissionMode::Plan)),
            "dont-ask" => Ok(ModeArg(PermissionMode::DontAsk)),
            other => Err(format!(
                "unknown mode '{other}'; valid modes: default, accept-edits, bypass, plan, dont-ask"
            )),
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "rust-claude", about = "Rust implementation of Claude Code")]
struct Cli {
    /// Prompt text (non-interactive mode when provided)
    prompt: Vec<String>,

    /// Permission mode: default, accept-edits, bypass, plan, dont-ask
    #[arg(short = 'm', long = "mode")]
    mode: Option<ModeArg>,

    /// Model to use (e.g. claude-sonnet-4-20250514, sonnet, opus)
    #[arg(long = "model")]
    model: Option<String>,

    /// Print response and exit (non-interactive / headless mode)
    #[arg(short = 'p', long = "print")]
    print: bool,

    /// Output format for non-interactive mode: text, json
    #[arg(long = "output-format", value_parser = ["text", "json"])]
    output_format: Option<String>,

    /// Maximum number of agentic turns
    #[arg(long = "max-turns")]
    max_turns: Option<usize>,

    /// Override the system prompt
    #[arg(long = "system-prompt")]
    system_prompt: Option<String>,

    /// Read system prompt from a file
    #[arg(long = "system-prompt-file")]
    system_prompt_file: Option<String>,

    /// Append text to the default system prompt
    #[arg(long = "append-system-prompt")]
    append_system_prompt: Option<String>,

    /// Read append system prompt from a file
    #[arg(long = "append-system-prompt-file")]
    append_system_prompt_file: Option<String>,

    /// Comma-separated list of allowed tool names (e.g. Bash,FileRead)
    #[arg(long = "allowed-tools", value_delimiter = ',')]
    allowed_tools: Option<Vec<String>>,

    /// Comma-separated list of denied tool names (e.g. Bash,FileWrite)
    #[arg(long = "disallowed-tools", value_delimiter = ',')]
    disallowed_tools: Option<Vec<String>>,

    /// Maximum output tokens
    #[arg(long = "max-tokens")]
    max_tokens: Option<u32>,

    /// Disable streaming
    #[arg(long = "no-stream")]
    no_stream: bool,

    /// Enable extended thinking (default: true for supported models)
    #[arg(long = "thinking", conflicts_with = "no_thinking")]
    thinking: bool,

    /// Disable extended thinking
    #[arg(long = "no-thinking")]
    no_thinking: bool,

    /// Verbose mode
    #[arg(long = "verbose")]
    verbose: bool,

    /// Continue the most recent session
    #[arg(long = "continue", short = 'c')]
    continue_session: bool,

    /// Path to a Claude Code settings.json file to load env from
    #[arg(long = "settings")]
    settings: Option<String>,
}

// ---------------------------------------------------------------------------
// Resolved configuration (unified priority chain result)
// ---------------------------------------------------------------------------

struct ResolvedConfig {
    api_key: String,
    model: String,
    model_setting: String,
    base_url: Option<String>,
    bearer_auth: bool,
    stream: bool,
    max_tokens: u32,
    system_prompt: Option<String>,
    permission_mode: PermissionMode,
    max_turns: Option<usize>,
    verbose: bool,
    print_mode: bool,
    output_json: bool,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
    always_allow: Vec<rust_claude_core::permission::PermissionRule>,
    always_deny: Vec<rust_claude_core::permission::PermissionRule>,
}

fn parse_bool_env(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

/// Resolve all configuration through the unified priority chain:
/// RUST_CLAUDE_* env > CLI flags > ANTHROPIC_* env (from settings.json + shell) > settings.json fields > config file > defaults
fn resolve_config(cli: &Cli, config: Config, settings: &ClaudeSettings) -> Result<ResolvedConfig> {
    // --- System prompt ---
    if cli.system_prompt.is_some() && cli.system_prompt_file.is_some() {
        return Err(anyhow!("--system-prompt and --system-prompt-file are mutually exclusive"));
    }
    let system_prompt = if let Some(ref path) = cli.system_prompt_file {
        Some(std::fs::read_to_string(path).map_err(|e| {
            anyhow!("failed to read --system-prompt-file '{}': {e}", path)
        })?)
    } else {
        cli.system_prompt.clone().or(config.system_prompt)
    };

    let append_prompt = if let Some(ref path) = cli.append_system_prompt_file {
        Some(std::fs::read_to_string(path).map_err(|e| {
            anyhow!("failed to read --append-system-prompt-file '{}': {e}", path)
        })?)
    } else {
        cli.append_system_prompt.clone()
    };
    let system_prompt = match (system_prompt, append_prompt) {
        (Some(base), Some(append)) => Some(format!("{base}\n\n{append}")),
        (Some(base), None) => Some(base),
        (None, Some(append)) => Some(append),
        (None, None) => None,
    };

    let max_tokens = cli.max_tokens.unwrap_or(config.max_tokens);

    let permission_mode = cli
        .mode
        .as_ref()
        .map(|m| m.0.clone())
        .unwrap_or(config.permission_mode);

    let model_setting = std::env::var("RUST_CLAUDE_MODEL_OVERRIDE")
        .ok()
        .or_else(|| cli.model.clone())
        .or_else(|| std::env::var("ANTHROPIC_MODEL").ok())
        .or_else(|| settings.model.clone())
        .unwrap_or(config.model);
    let model = get_runtime_main_loop_model(&model_setting, permission_mode, false);

    let base_url = std::env::var("RUST_CLAUDE_BASE_URL")
        .ok()
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        .or(config.base_url);

    let bearer_auth = std::env::var("RUST_CLAUDE_BEARER_AUTH")
        .ok()
        .and_then(|v| parse_bool_env(&v))
        .unwrap_or(config.bearer_auth);

    let stream = std::env::var("RUST_CLAUDE_STREAM")
        .ok()
        .and_then(|v| parse_bool_env(&v))
        .unwrap_or_else(|| if cli.no_stream { false } else { config.stream });

    let print_mode = cli.print || !cli.prompt.is_empty();
    let output_json = cli.output_format.as_deref() == Some("json");
    let allowed_tools = cli.allowed_tools.clone().unwrap_or_default();
    let disallowed_tools = cli.disallowed_tools.clone().unwrap_or_default();

    Ok(ResolvedConfig {
        api_key: config.api_key,
        model,
        model_setting,
        base_url,
        bearer_auth,
        stream,
        max_tokens,
        system_prompt,
        permission_mode,
        max_turns: cli.max_turns,
        verbose: cli.verbose,
        print_mode,
        output_json,
        allowed_tools,
        disallowed_tools,
        always_allow: config.always_allow,
        always_deny: config.always_deny,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    println!("rust-claude-code: Rust implementation of Claude Code");

    let settings = match &cli.settings {
        Some(path) => ClaudeSettings::load_from(std::path::Path::new(path))?,
        None => ClaudeSettings::load().unwrap_or_default(),
    };
    settings.apply_env();

    let config = match Config::load() {
        Ok(config) => config,
        Err(ConfigError::MissingApiKey) => {
            if let Some(ref helper) = settings.api_key_helper {
                match run_api_key_helper(helper) {
                    Ok(credential) => Config::with_credential(credential, true),
                    Err(e) => {
                        eprintln!("apiKeyHelper failed: {e}");
                        return Err(anyhow!(
                            "No API credential found. Set ANTHROPIC_API_KEY, ANTHROPIC_AUTH_TOKEN, or configure apiKeyHelper in settings.json"
                        ));
                    }
                }
            } else {
                return Err(anyhow!(
                    "No API credential found. Set ANTHROPIC_API_KEY, ANTHROPIC_AUTH_TOKEN, or configure apiKeyHelper in settings.json"
                ));
            }
        }
        Err(error) => return Err(error.into()),
    };

    let resolved = resolve_config(&cli, config, &settings)?;

    if resolved.verbose {
        println!("cwd: {}", cwd.display());
        println!("model: {}", resolved.model);
        println!("model_setting: {}", resolved.model_setting);
        println!("max_tokens: {}", resolved.max_tokens);
        println!("stream: {}", resolved.stream);
        println!("permission_mode: {:?}", resolved.permission_mode);
        println!("max_turns: {:?}", resolved.max_turns);
        if resolved.allowed_tools.is_empty() {
            println!("allowed_tools: (all)");
        } else {
            println!("allowed_tools: {:?}", resolved.allowed_tools);
        }
        println!("disallowed_tools: {:?}", resolved.disallowed_tools);
        println!("always_allow_rules: {}", resolved.always_allow.len());
        println!("always_deny_rules: {}", resolved.always_deny.len());
    }

    let mut state = AppState::new(cwd.clone());
    state.session.model = resolved.model.clone();
    state.session.model_setting = resolved.model_setting.clone();
    state.session.system_prompt = resolved.system_prompt.clone();
    state.session.max_tokens = resolved.max_tokens;
    state.session.stream = resolved.stream;
    // --no-thinking explicitly disables, --thinking explicitly enables, otherwise default (true)
    if cli.no_thinking {
        state.session.thinking_enabled = false;
    } else if cli.thinking {
        state.session.thinking_enabled = true;
    }
    state.permission_mode = resolved.permission_mode;
    state.always_allow_rules = resolved.always_allow.clone();
    state.always_deny_rules = resolved.always_deny.clone();

    if cli.continue_session {
        match session::load_latest_session() {
            Ok(Some(prev)) => {
                state.messages = prev.messages;
                if !prev.model_setting.is_empty() {
                    state.session.model_setting = prev.model_setting;
                }
                state.session.model = get_runtime_main_loop_model(
                    &state.session.model_setting,
                    state.permission_mode,
                    false,
                );
                println!("Continuing session {} ({} messages)", prev.id, state.messages.len());
            }
            Ok(None) => {
                println!("No previous session found. Starting fresh.");
            }
            Err(e) => {
                eprintln!("Warning: failed to load previous session: {e}");
            }
        }
    }

    let app_state = Arc::new(Mutex::new(state));

    let claude_md_files = claude_md::discover_claude_md(&cwd);
    if resolved.verbose && !claude_md_files.is_empty() {
        println!("Discovered {} CLAUDE.md file(s):", claude_md_files.len());
        for f in &claude_md_files {
            println!("  - {} ({} chars)", f.path.display(), f.content.len());
        }
    }

    if resolved.system_prompt.is_none() {
        let tools_for_prompt =
            build_filtered_tools(&resolved.allowed_tools, &resolved.disallowed_tools);
        let composed =
            system_prompt::build_system_prompt(&cwd, &tools_for_prompt, &claude_md_files, None);
        let mut state = app_state.lock().await;
        state.session.system_prompt = Some(composed);
    }

    if resolved.print_mode {
        let prompt = cli.prompt.join(" ");
        let client =
            build_client(&resolved.api_key, resolved.base_url.clone(), resolved.bearer_auth)?;
        let mut tools = build_tools();
        tools.apply_tool_filters(&resolved.allowed_tools, &resolved.disallowed_tools);
        let mut query_loop =
            QueryLoop::new(client, tools).with_compaction_config(CompactionConfig::default());
        if let Some(max_turns) = resolved.max_turns {
            query_loop = query_loop.with_max_rounds(max_turns);
        }
        let final_message = query_loop.run(app_state, prompt).await?;

        if resolved.output_json {
            let json = serde_json::to_string_pretty(&final_message)?;
            println!("{json}");
        } else {
            for block in final_message.content {
                if let ContentBlock::Text { text } = block {
                    println!("{text}");
                }
            }
        }
        Ok(())
    } else {
        let allowed_tools = resolved.allowed_tools.clone();
        let disallowed_tools = resolved.disallowed_tools.clone();
        run_tui(app_state, allowed_tools, disallowed_tools).await
    }
}

fn run_api_key_helper(command: &str) -> Result<String> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| anyhow!("failed to execute apiKeyHelper: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("apiKeyHelper exited with {}: {stderr}", output.status));
    }

    let credential = String::from_utf8(output.stdout)
        .map_err(|e| anyhow!("apiKeyHelper output is not valid UTF-8: {e}"))?
        .trim()
        .to_string();

    if credential.is_empty() {
        return Err(anyhow!("apiKeyHelper returned empty output"));
    }

    Ok(credential)
}

fn build_client(
    api_key: &str,
    base_url_override: Option<String>,
    bearer_auth: bool,
) -> Result<AnthropicClient> {
    let mut client = AnthropicClient::new(api_key.to_string())?;
    if let Some(base_url) = base_url_override {
        client = client.with_base_url(base_url);
    }
    if bearer_auth {
        client = client.with_bearer_auth();
    }
    Ok(client)
}

fn build_tools() -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(BashTool::new());
    tools.register(FileReadTool::new());
    tools.register(FileEditTool::new());
    tools.register(FileWriteTool::new());
    tools.register(GlobTool::new());
    tools.register(GrepTool::new());
    tools.register(TodoWriteTool::new());
    tools
}

fn build_filtered_tools(allowed: &[String], disallowed: &[String]) -> ToolRegistry {
    let mut tools = build_tools();
    tools.apply_tool_filters(allowed, disallowed);
    tools
}

async fn run_tui(
    app_state: Arc<Mutex<AppState>>,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
) -> Result<()> {
    let (model, model_setting, permission_mode) = {
        let state = app_state.lock().await;
        (
            get_runtime_main_loop_model(
                &state.session.model_setting,
                state.permission_mode,
                false,
            ),
            state.session.model_setting.clone(),
            format!("{:?}", state.permission_mode),
        )
    };

    let config = Config::load().map_err(|e| anyhow!("failed to load config for TUI: {e}"))?;
    let base_url = std::env::var("RUST_CLAUDE_BASE_URL")
        .ok()
        .or_else(|| std::env::var("ANTHROPIC_BASE_URL").ok())
        .or(config.base_url);
    let bearer_auth = std::env::var("RUST_CLAUDE_BEARER_AUTH")
        .ok()
        .and_then(|v| parse_bool_env(&v))
        .unwrap_or(config.bearer_auth);

    let (event_tx, event_rx) = mpsc::channel(128);
    let (user_tx, mut user_rx) = mpsc::channel::<UserCommand>(16);

    let bridge = TuiBridge::new(event_tx);
    let worker_bridge = bridge.clone();
    let worker_state = app_state.clone();

    worker_bridge
        .send_status_update(&model, &model_setting, &permission_mode)
        .await;

    tokio::spawn(async move {
        let compaction_config = CompactionConfig::default();
        let mut active_query_task: Option<tokio::task::JoinHandle<()>> = None;

        while let Some(command) = user_rx.recv().await {
            match command {
                UserCommand::Compact => {
                    let client = match build_client(&config.api_key, base_url.clone(), bearer_auth) {
                        Ok(client) => client,
                        Err(error) => {
                            worker_bridge.send_error(&error.to_string()).await;
                            continue;
                        }
                    };

                    worker_bridge.send_compaction_start().await;
                    let service = CompactionService::new(client, compaction_config.clone());
                    match service.force_compact(&worker_state).await {
                        Ok(result) => {
                            worker_bridge.send_compaction_complete(result).await;
                        }
                        Err(e) => {
                            worker_bridge
                                .send_error(&format!("Compaction failed: {e}"))
                                .await;
                        }
                    }

                    let state_snapshot = worker_state.lock().await;
                    let mut session_file = SessionFile::new(
                        &state_snapshot.session.model,
                        &state_snapshot.session.model_setting,
                        &state_snapshot.cwd,
                    );
                    session_file.messages = state_snapshot.messages.clone();
                    drop(state_snapshot);
                    if let Err(e) = session_file.save() {
                        worker_bridge
                            .send_error(&format!("Warning: failed to save session: {e}"))
                            .await;
                    }
                }
                UserCommand::SetMode(mode_str) => {
                    let mode = match mode_str.as_str() {
                        "default" => PermissionMode::Default,
                        "accept-edits" => PermissionMode::AcceptEdits,
                        "bypass" => PermissionMode::BypassPermissions,
                        "plan" => PermissionMode::Plan,
                        "dont-ask" => PermissionMode::DontAsk,
                        _ => {
                            worker_bridge.send_error("Unknown mode request").await;
                            continue;
                        }
                    };

                    let (runtime_model, model_setting, permission_mode_display) = {
                        let mut state = worker_state.lock().await;
                        state.permission_mode = mode;
                        state.session.model = get_runtime_main_loop_model(
                            &state.session.model_setting,
                            state.permission_mode,
                            state
                                .most_recent_assistant_usage()
                                .is_some_and(rust_claude_core::model::usage_exceeds_200k_tokens),
                        );
                        (
                            state.session.model.clone(),
                            state.session.model_setting.clone(),
                            format!("{:?}", state.permission_mode),
                        )
                    };

                    worker_bridge
                        .send_status_update(&runtime_model, &model_setting, &permission_mode_display)
                        .await;
                    worker_bridge
                        .send_assistant_message(&format!("Permission mode switched to: {mode_str}"))
                        .await;
                }
                UserCommand::SetModel(model_str) => {
                    if model_str.trim().is_empty() {
                        worker_bridge.send_error("Model cannot be empty").await;
                        continue;
                    }

                    let (runtime_model, model_setting, permission_mode_display) = {
                        let mut state = worker_state.lock().await;
                        state.session.model_setting = model_str.trim().to_string();
                        state.session.model = get_runtime_main_loop_model(
                            &state.session.model_setting,
                            state.permission_mode,
                            state
                                .most_recent_assistant_usage()
                                .is_some_and(rust_claude_core::model::usage_exceeds_200k_tokens),
                        );
                        (
                            state.session.model.clone(),
                            state.session.model_setting.clone(),
                            format!("{:?}", state.permission_mode),
                        )
                    };

                    worker_bridge
                        .send_status_update(&runtime_model, &model_setting, &permission_mode_display)
                        .await;
                    worker_bridge
                        .send_assistant_message(&format!("Model switched to: {model_setting}"))
                        .await;
                }
                UserCommand::CancelStream => {
                    if let Some(handle) = active_query_task.take() {
                        handle.abort();
                        worker_bridge.send_stream_cancelled().await;
                    }
                }
                UserCommand::Prompt(prompt) => {
                    let client = match build_client(&config.api_key, base_url.clone(), bearer_auth) {
                        Ok(client) => client,
                        Err(error) => {
                            worker_bridge.send_error(&error.to_string()).await;
                            continue;
                        }
                    };

                    let tools = build_filtered_tools(&allowed_tools, &disallowed_tools);
                    let query_loop = QueryLoop::new(client, tools)
                        .with_bridge(worker_bridge.clone())
                        .with_compaction_config(compaction_config.clone());
                    let worker_bridge_clone = worker_bridge.clone();
                    let worker_state_clone = worker_state.clone();

                    let handle = tokio::spawn(async move {
                        match query_loop.run(worker_state_clone.clone(), prompt).await {
                            Ok(final_message) => {
                                let text = final_message
                                    .content
                                    .into_iter()
                                    .filter_map(|block| match block {
                                        ContentBlock::Text { text } => Some(text),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");

                                let (usage, runtime_model, model_setting, permission_mode_display, stream_enabled) = {
                                    let state = worker_state_clone.lock().await;
                                    (
                                        state.total_usage.clone(),
                                        get_runtime_main_loop_model(
                                            &state.session.model_setting,
                                            state.permission_mode,
                                            state
                                                .most_recent_assistant_usage()
                                                .is_some_and(rust_claude_core::model::usage_exceeds_200k_tokens),
                                        ),
                                        state.session.model_setting.clone(),
                                        format!("{:?}", state.permission_mode),
                                        state.session.stream,
                                    )
                                };

                                if !stream_enabled {
                                    if !text.is_empty() {
                                        worker_bridge_clone.send_assistant_message(&text).await;
                                    } else {
                                        worker_bridge_clone.send_assistant_message("(no text content)").await;
                                    }
                                }

                                worker_bridge_clone
                                    .send_usage_update(
                                        usage.input_tokens as u64,
                                        usage.output_tokens as u64,
                                        usage.cache_read_input_tokens as u64,
                                        usage.cache_creation_input_tokens as u64,
                                    )
                                    .await;
                                worker_bridge_clone
                                    .send_status_update(&runtime_model, &model_setting, &permission_mode_display)
                                    .await;
                            }
                            Err(error) => {
                                worker_bridge_clone.send_error(&error.to_string()).await;
                            }
                        }

                        let state_snapshot = worker_state_clone.lock().await;
                        let mut session_file = SessionFile::new(
                            &state_snapshot.session.model,
                            &state_snapshot.session.model_setting,
                            &state_snapshot.cwd,
                        );
                        session_file.messages = state_snapshot.messages.clone();
                        drop(state_snapshot);
                        if let Err(e) = session_file.save() {
                            worker_bridge_clone
                                .send_error(&format!("Warning: failed to save session: {e}"))
                                .await;
                        }
                    });

                    active_query_task = Some(handle);
                }
            }
        }
    });

    let mut terminal_guard = TerminalGuard::new()?;
    let mut app = App::new(model, model_setting, permission_mode);
    app.run(terminal_guard.terminal_mut(), event_rx, user_tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_config_parses_model_aliases_before_storing_session_model() {
        let cli = Cli {
            prompt: vec![],
            mode: None,
            model: Some("opus[1m]".to_string()),
            print: false,
            output_format: None,
            max_turns: None,
            system_prompt: None,
            system_prompt_file: None,
            append_system_prompt: None,
            append_system_prompt_file: None,
            allowed_tools: None,
            disallowed_tools: None,
            max_tokens: None,
            no_stream: false,
            thinking: false,
            no_thinking: false,
            verbose: false,
            continue_session: false,
            settings: None,
        };
        let config = Config::with_credential("test-key".to_string(), false);
        let settings = ClaudeSettings::default();

        let resolved = resolve_config(&cli, config, &settings).unwrap();
        assert_eq!(resolved.model_setting, "opus[1m]");
        assert_eq!(resolved.model, "claude-opus-4-6[1m]");
    }

    #[test]
    fn restored_assistant_message_usage_can_drive_opusplan_fallback() {
        let mut state = AppState::new(std::path::PathBuf::from("/tmp"));
        state.permission_mode = PermissionMode::Plan;
        state.session.model_setting = "opusplan".to_string();
        state.add_assistant_message(rust_claude_core::message::Message::assistant_with_usage(
            vec![ContentBlock::text("previous")],
            rust_claude_core::message::Usage {
                input_tokens: 150_000,
                output_tokens: 40_000,
                cache_creation_input_tokens: 10_001,
                cache_read_input_tokens: 0,
            },
        ));

        let runtime_model = get_runtime_main_loop_model(
            &state.session.model_setting,
            state.permission_mode,
            state
                .most_recent_assistant_usage()
                .is_some_and(rust_claude_core::model::usage_exceeds_200k_tokens),
        );

        assert_eq!(runtime_model, "claude-sonnet-4-6");
    }
}

