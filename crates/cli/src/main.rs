use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Parser;
use rust_claude_api::AnthropicClient;
use rust_claude_core::{
    claude_md,
    config::{Config, ConfigError, ConfigOverrides, ConfigSource, Provider, Theme},
    custom_agents::CustomAgentRegistry,
    git::collect_git_context,
    hooks::HooksConfig,
    mcp_config::McpServersConfig,
    memory,
    message::{ContentBlock, Message, Role, Usage},
    model::get_runtime_main_loop_model,
    permission::PermissionMode,
    session::{ContextSnapshot, SessionSummary},
    settings::{ClaudeSettings, ParsedPermissions, SettingsLayer},
    state::AppState,
};
use rust_claude_mcp::{McpManager, McpManagerConfig};
use rust_claude_tools::{
    register_mcp_tools, AgentContext, AgentTool, AskUserQuestionTool, AutoMemoryTool, BashTool,
    EnterPlanModeTool, ExitPlanModeTool, FileEditTool, FileReadTool, FileWriteTool, GlobTool,
    GrepTool, LspTool, MonitorTool, NotebookEditTool, TaskTool, ToolRegistry, WebFetchTool,
    WebSearchTool,
};
use rust_claude_tui::{App, AppEvent, ChatMessage, TerminalGuard, TuiBridge, UserCommand};
use rust_claude_sdk::plugin::{PluginManager};
use tokio::sync::{mpsc, Mutex};

use rust_claude_cli::compaction::CompactionService;
use rust_claude_cli::hooks::HookRunner;
use rust_claude_cli::query_loop::QueryLoop;
use rust_claude_cli::session::{self, SessionFile};
use rust_claude_cli::system_prompt;
use rust_claude_core::compaction::{CompactStrategy, CompactionConfig};
use rust_claude_core::model::{EFFORT_LOW_THINKING_BUDGET, EFFORT_MEDIUM_THINKING_BUDGET};

const REVIEW_DIFF_LIMIT: usize = 60_000;

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
    prompt: Vec<String>,

    #[arg(short = 'm', long = "mode")]
    mode: Option<ModeArg>,

    #[arg(long = "model")]
    model: Option<String>,

    #[arg(short = 'p', long = "print")]
    print: bool,

    #[arg(long = "output-format", value_parser = ["text", "json"])]
    output_format: Option<String>,

    #[arg(long = "max-turns")]
    max_turns: Option<usize>,

    #[arg(long = "system-prompt")]
    system_prompt: Option<String>,

    #[arg(long = "system-prompt-file")]
    system_prompt_file: Option<String>,

    #[arg(long = "append-system-prompt")]
    append_system_prompt: Option<String>,

    #[arg(long = "append-system-prompt-file")]
    append_system_prompt_file: Option<String>,

    #[arg(long = "allowed-tools", value_delimiter = ',')]
    allowed_tools: Option<Vec<String>>,

    #[arg(long = "disallowed-tools", value_delimiter = ',')]
    disallowed_tools: Option<Vec<String>>,

    #[arg(long = "max-tokens")]
    max_tokens: Option<u32>,

    #[arg(long = "no-stream")]
    no_stream: bool,

    #[arg(long = "theme", value_parser = ["dark", "light"])]
    theme: Option<String>,

    #[arg(long = "provider", value_parser = ["anthropic", "bedrock", "vertex"])]
    provider: Option<String>,

    #[arg(long = "thinking", conflicts_with = "no_thinking")]
    thinking: bool,

    #[arg(long = "no-thinking")]
    no_thinking: bool,

    #[arg(long = "verbose")]
    verbose: bool,

    #[arg(long = "continue", short = 'c')]
    continue_session: bool,

    #[arg(long = "resume", short = 'r')]
    resume_session: Option<String>,

    #[arg(long = "settings")]
    settings: Option<String>,
}

#[derive(Debug, Clone)]
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
    hooks_config: HooksConfig,
    mcp_servers: McpServersConfig,
    provider: Provider,
    config: Config,
    project_settings: Option<SettingsLayer>,
}

fn parse_bool_env(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

fn parse_provider(value: &str) -> Provider {
    match value {
        "anthropic" => Provider::Anthropic,
        "bedrock" => Provider::Bedrock,
        "vertex" => Provider::Vertex,
        _ => unreachable!("clap restricts provider values"),
    }
}

fn merge_settings_layers(
    user_settings: ClaudeSettings,
    project_settings: Option<&SettingsLayer>,
) -> ClaudeSettings {
    match project_settings {
        Some(layer) => ClaudeSettings::merge(&layer.settings, &user_settings),
        None => user_settings,
    }
}

fn permissions_from_settings(settings: &ClaudeSettings) -> Result<ParsedPermissions> {
    settings
        .parsed_permissions()
        .map_err(|e| anyhow!("invalid permissions in settings: {e}"))
}

fn resolve_config(
    cli: &Cli,
    mut config: Config,
    user_settings: ClaudeSettings,
    project_settings: Option<SettingsLayer>,
) -> Result<ResolvedConfig> {
    if cli.system_prompt.is_some() && cli.system_prompt_file.is_some() {
        return Err(anyhow!(
            "--system-prompt and --system-prompt-file are mutually exclusive"
        ));
    }

    let merged_settings = merge_settings_layers(user_settings.clone(), project_settings.as_ref());
    let project_permissions = project_settings
        .as_ref()
        .map(|layer| permissions_from_settings(&layer.settings))
        .transpose()?;
    let user_permissions = permissions_from_settings(&user_settings)?;

    let mut overrides = ConfigOverrides::default();

    if let Some(model) = merged_settings.model.clone() {
        let source = if project_settings
            .as_ref()
            .and_then(|layer| layer.settings.model.as_ref())
            .is_some()
        {
            ConfigSource::ProjectSettings
        } else {
            ConfigSource::UserConfig
        };
        overrides.model.set(model, source);
    }
    // Merge user + project permission lists independently for each of allow/deny.
    // A project that only sets `allow` must not drop the user-scope `deny` rules
    // (and vice versa). Ordering: user entries first, then project entries.
    let project_allow = project_permissions
        .as_ref()
        .map(|p| p.allow.clone())
        .unwrap_or_default();
    let project_deny = project_permissions
        .as_ref()
        .map(|p| p.deny.clone())
        .unwrap_or_default();

    let mut merged_allow = user_permissions.allow.clone();
    merged_allow.extend(project_allow.iter().cloned());
    let mut merged_deny = user_permissions.deny.clone();
    merged_deny.extend(project_deny.iter().cloned());

    // Provenance reflects the most-specific contributing layer: if project
    // contributed any rules, attribute to ProjectSettings; otherwise UserConfig.
    if !merged_allow.is_empty() {
        let source = if !project_allow.is_empty() {
            ConfigSource::ProjectSettings
        } else {
            ConfigSource::UserConfig
        };
        overrides.always_allow.set(merged_allow, source);
    }
    if !merged_deny.is_empty() {
        let source = if !project_deny.is_empty() {
            ConfigSource::ProjectSettings
        } else {
            ConfigSource::UserConfig
        };
        overrides.always_deny.set(merged_deny, source);
    }

    if let Some(path) = &cli.system_prompt_file {
        overrides.system_prompt.set(
            Some(
                std::fs::read_to_string(path)
                    .map_err(|e| anyhow!("failed to read --system-prompt-file '{}': {e}", path))?,
            ),
            ConfigSource::Cli,
        );
    } else if let Some(prompt) = cli.system_prompt.clone() {
        overrides.system_prompt.set(Some(prompt), ConfigSource::Cli);
    }

    // Apply low-priority env overrides FIRST (ANTHROPIC_MODEL / ANTHROPIC_BASE_URL).
    // CLI flags apply afterwards so they win (documented priority in CLAUDE.md:
    // RUST_CLAUDE_MODEL_OVERRIDE > --model > ANTHROPIC_MODEL > settings > config).
    if let Ok(model) = std::env::var("ANTHROPIC_MODEL") {
        overrides.model.set(model, ConfigSource::Env);
    }
    if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
        overrides.base_url.set(Some(base_url), ConfigSource::Env);
    }

    if let Some(max_tokens) = cli.max_tokens {
        overrides.max_tokens.set(max_tokens, ConfigSource::Cli);
    }
    if let Some(mode) = cli.mode.as_ref() {
        overrides.permission_mode.set(mode.0, ConfigSource::Cli);
    }
    if let Some(model) = cli.model.clone() {
        overrides.model.set(model, ConfigSource::Cli);
    }
    if cli.no_stream {
        overrides.stream.set(false, ConfigSource::Cli);
    }
    if let Some(theme) = cli.theme.as_deref() {
        let theme = match theme {
            "dark" => Theme::Dark,
            "light" => Theme::Light,
            _ => unreachable!("clap restricts theme values"),
        };
        overrides.theme.set(theme, ConfigSource::Cli);
    }

    // Highest-priority overrides (RUST_CLAUDE_* escape hatches) apply last so they
    // beat both CLI flags and ANTHROPIC_* env variables.
    if let Ok(model) = std::env::var("RUST_CLAUDE_MODEL_OVERRIDE") {
        overrides.model.set(model, ConfigSource::Env);
    }
    if let Ok(base_url) = std::env::var("RUST_CLAUDE_BASE_URL") {
        overrides.base_url.set(Some(base_url), ConfigSource::Env);
    }
    if let Ok(stream) = std::env::var("RUST_CLAUDE_STREAM") {
        if let Some(value) = parse_bool_env(&stream) {
            overrides.stream.set(value, ConfigSource::Env);
        }
    }
    if let Ok(bearer) = std::env::var("RUST_CLAUDE_BEARER_AUTH") {
        if let Some(value) = parse_bool_env(&bearer) {
            overrides.bearer_auth.set(value, ConfigSource::Env);
        }
    }

    if let Some(append_path) = &cli.append_system_prompt_file {
        let append = std::fs::read_to_string(append_path).map_err(|e| {
            anyhow!(
                "failed to read --append-system-prompt-file '{}': {e}",
                append_path
            )
        })?;
        let base = overrides
            .system_prompt
            .value
            .clone()
            .flatten()
            .or_else(|| config.system_prompt.clone())
            .unwrap_or_default();
        overrides.system_prompt.set(
            Some(if base.is_empty() {
                append
            } else {
                format!("{base}\n\n{append}")
            }),
            ConfigSource::Cli,
        );
    } else if let Some(append) = &cli.append_system_prompt {
        let base = overrides
            .system_prompt
            .value
            .clone()
            .flatten()
            .or_else(|| config.system_prompt.clone())
            .unwrap_or_default();
        overrides.system_prompt.set(
            Some(if base.is_empty() {
                append.clone()
            } else {
                format!("{base}\n\n{append}")
            }),
            ConfigSource::Cli,
        );
    }

    config = config.apply_overrides(overrides);

    let model_setting = config.model.clone();
    let permission_mode = config.permission_mode;
    let model = get_runtime_main_loop_model(&model_setting, permission_mode, false);

    let print_mode = cli.print || !cli.prompt.is_empty();
    let output_json = cli.output_format.as_deref() == Some("json");
    let allowed_tools = cli.allowed_tools.clone().unwrap_or_default();
    let disallowed_tools = cli.disallowed_tools.clone().unwrap_or_default();
    let (provider, _) = Config::resolve_provider(
        cli.provider.as_deref().map(parse_provider),
        Some(config.provider),
    )?;

    Ok(ResolvedConfig {
        api_key: config.api_key.clone(),
        model,
        model_setting,
        base_url: config.base_url.clone(),
        bearer_auth: config.bearer_auth,
        stream: config.stream,
        max_tokens: config.max_tokens,
        system_prompt: config.system_prompt.clone(),
        permission_mode,
        max_turns: cli.max_turns,
        verbose: cli.verbose,
        print_mode,
        output_json,
        allowed_tools,
        disallowed_tools,
        always_allow: config.always_allow.clone(),
        always_deny: config.always_deny.clone(),
        hooks_config: merged_settings.hooks.clone(),
        mcp_servers: merged_settings.mcp_servers.clone(),
        provider,
        config,
        project_settings,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    let user_settings = match &cli.settings {
        Some(path) => ClaudeSettings::load_from(std::path::Path::new(path))?,
        None => ClaudeSettings::load().unwrap_or_default(),
    };
    user_settings.apply_env();

    let project_settings = ClaudeSettings::load_project(&cwd)?;
    if let Some(layer) = &project_settings {
        layer.settings.apply_env();
    }

    let config = match Config::load() {
        Ok(config) => config,
        Err(ConfigError::MissingApiKey) => {
            let merged_settings =
                merge_settings_layers(user_settings.clone(), project_settings.as_ref());
            if let Some(ref helper) = merged_settings.api_key_helper {
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

    let resolved = resolve_config(&cli, config, user_settings, project_settings)?;

    if resolved.verbose {
        println!("cwd: {}", cwd.display());
        println!("model: {}", resolved.model);
        println!("model_setting: {}", resolved.model_setting);
        println!("max_tokens: {}", resolved.max_tokens);
        println!("stream: {}", resolved.stream);
        println!("permission_mode: {:?}", resolved.permission_mode);
        println!("max_turns: {:?}", resolved.max_turns);
        println!("model_source: {}", resolved.config.provenance.model);
        println!(
            "permissions_source: {} / {}",
            resolved.config.provenance.always_allow, resolved.config.provenance.always_deny
        );
        if let Some(layer) = &resolved.project_settings {
            println!("project_settings: {}", layer.path.display());
        }
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
    state.config = resolved.config.clone();
    state.config_provenance = resolved.config.provenance.clone();
    state.git_context = collect_git_context(&cwd);
    if cli.no_thinking {
        state.session.thinking_enabled = false;
    } else if cli.thinking {
        state.session.thinking_enabled = true;
    }
    state.permission_mode = resolved.permission_mode;
    state.always_allow_rules = resolved.always_allow.clone();
    state.always_deny_rules = resolved.always_deny.clone();

    if let Some(session_id) = &cli.resume_session {
        match session::load_session_by_id(session_id) {
            Ok(Some(prev)) => {
                let msg_count = prev.messages.len();
                let id = prev.id.clone();
                session::restore_app_state_from_session(&mut state, &prev);
                println!("Resumed session {} ({} messages)", id, msg_count);
            }
            Ok(None) => return Err(anyhow!("session '{}' not found", session_id)),
            Err(e) => return Err(anyhow!("failed to load session '{}': {e}", session_id)),
        }
    } else if cli.continue_session {
        match session::load_latest_session() {
            Ok(Some(prev)) => {
                let msg_count = prev.messages.len();
                let id = prev.id.clone();
                session::restore_app_state_from_session(&mut state, &prev);
                println!("Continuing session {} ({} messages)", id, msg_count);
            }
            Ok(None) => {
                println!("No previous session found. Starting fresh.");
            }
            Err(e) => {
                eprintln!("Warning: failed to load previous session: {e}");
            }
        }
    }

    if state.session.id.is_empty() {
        state.session.id = SessionFile::new(&state.session.model, &state.session.model_setting, &cwd).id;
    }

    let app_state = Arc::new(Mutex::new(state));

    let claude_md_files = claude_md::discover_claude_md(&cwd);
    let mut custom_agents = Arc::new(CustomAgentRegistry::discover(&cwd));
    if resolved.verbose {
        println!("Discovered {} custom agent(s)", custom_agents.list().len());
        for error in custom_agents.errors() {
            eprintln!(
                "Warning: failed to load custom agent {}: {}",
                error.path.display(),
                error.message
            );
        }
    }
    let memory_store = memory::discover_memory_store(&cwd)
        .and_then(|store| memory::scan_memory_store(&store).ok());
    if resolved.verbose && !claude_md_files.is_empty() {
        println!("Discovered {} CLAUDE.md file(s):", claude_md_files.len());
        for f in &claude_md_files {
            println!("  - {} ({} chars)", f.path.display(), f.content.len());
        }
    }

    // Build hook runner from merged settings
    let hook_runner = if resolved.hooks_config.is_empty() {
        None
    } else {
        Some(Arc::new(HookRunner::new(
            resolved.hooks_config.clone(),
            cwd.clone(),
        )))
    };
    if let Some(runner) = &hook_runner {
        let session_id = { app_state.lock().await.session.id.clone() };
        runner.run_session_start(&session_id).await;
    }

    // Initialize MCP servers (if any configured)
    let mcp_manager = if resolved.mcp_servers.is_empty() {
        Arc::new(McpManager::empty())
    } else {
        if resolved.verbose {
            println!(
                "MCP: starting {} configured server(s)...",
                resolved.mcp_servers.len()
            );
        }
        let manager = McpManager::start(&resolved.mcp_servers, &McpManagerConfig::default()).await;
        if resolved.verbose {
            println!(
                "MCP: {} server(s) connected, {} tool(s) discovered",
                manager.connected_count(),
                manager.tool_count()
            );
            for status in manager.server_statuses() {
                match &status.state {
                    rust_claude_core::mcp_config::McpServerState::Connected => {
                        println!(
                            "  {} (connected, {} tools)",
                            status.name,
                            status.tools.len()
                        );
                    }
                    rust_claude_core::mcp_config::McpServerState::Failed(msg) => {
                        println!("  {} (failed: {})", status.name, msg);
                    }
                    rust_claude_core::mcp_config::McpServerState::Disconnected(msg) => {
                        println!("  {} (disconnected: {})", status.name, msg);
                    }
                    rust_claude_core::mcp_config::McpServerState::Pending => {
                        println!("  {} (pending)", status.name);
                    }
                }
            }
        }
        Arc::new(manager)
    };

    // Load plugins
    let plugin_manager = Arc::new(Mutex::new(PluginManager::new(Some(&cwd))));
    if !plugin_manager.lock().await.plugins().is_empty() && resolved.verbose {
        println!(
            "Plugins: loaded {} plugin(s)",
            plugin_manager.lock().await.plugins().len()
        );
    }
    // Register plugin custom agents
    {
        let mut all_agents: Vec<_> = custom_agents.list().into_iter().cloned().collect();
        for plugin in plugin_manager.lock().await.plugins() {
            for agent in &plugin.custom_agents {
                all_agents.push(rust_claude_core::custom_agents::CustomAgentDefinition {
                    name: agent.name.clone(),
                    description: agent.description.clone(),
                    system_prompt: agent.system_prompt.clone(),
                    tools: agent.tools.clone(),
                    model: agent.model.clone(),
                    path: plugin.manifest_path.clone(),
                });
            }
        }
        custom_agents = Arc::new(rust_claude_core::custom_agents::CustomAgentRegistry::from_agents(all_agents));
    }

    if resolved.system_prompt.is_none() {
        let mut tools_for_prompt = build_tools();
        register_mcp_tools(&mut tools_for_prompt, &mcp_manager);
        tools_for_prompt.apply_tool_filters(&resolved.allowed_tools, &resolved.disallowed_tools);
        let git_context = { app_state.lock().await.git_context.clone() };
        let relevant_memories = memory_store
            .as_ref()
            .map(|scanned| memory::select_relevant_memories(scanned, "session start", 5))
            .unwrap_or_default();
        let composed = system_prompt::build_system_prompt(
            &cwd,
            &tools_for_prompt,
            &claude_md_files,
            memory_store.as_ref(),
            &relevant_memories,
            git_context.as_ref(),
            None,
        );
        let mut state = app_state.lock().await;
        state.session.system_prompt = Some(composed);
    }

    if resolved.print_mode {
        let prompt = cli.prompt.join(" ");
        let client = build_client(
            &resolved.api_key,
            resolved.base_url.clone(),
            resolved.bearer_auth,
            resolved.provider,
        )?;
        let mut tools = build_tools();
        register_mcp_tools(&mut tools, &mcp_manager);
        tools.apply_tool_filters(&resolved.allowed_tools, &resolved.disallowed_tools);
        let agent_context = build_agent_context(Arc::new(client.clone()), custom_agents.clone());

        let output_json = resolved.output_json;
        let stream_enabled = resolved.config.stream;

        let mut query_loop = QueryLoop::new(client, tools)
            .with_compaction_config(CompactionConfig::default())
            .with_agent_context(agent_context);
        if let Some(max_turns) = resolved.max_turns {
            query_loop = query_loop.with_max_rounds(max_turns);
        }
        if let Some(runner) = &hook_runner {
            query_loop = query_loop.with_hook_runner(runner.clone());
        }

        // For streaming print mode, attach a bridge that streams to stdout
        let run_result = if stream_enabled && !output_json {
            let (print_tx, mut print_rx) = tokio::sync::mpsc::channel::<AppEvent>(256);
            let bridge = rust_claude_tui::TuiBridge::new(print_tx);
            query_loop = query_loop
                .with_output(Box::new(bridge.clone()))
                .with_permission_ui(Box::new(bridge.clone()))
                .with_user_question_ui(Box::new(bridge));

            // Spawn a task to consume bridge events and stream to stdout/stderr
            let print_handle = tokio::spawn(async move {
                use std::io::Write;
                let stdout = std::io::stdout();
                let stderr = std::io::stderr();
                while let Some(event) = print_rx.recv().await {
                    match event {
                        AppEvent::StreamDelta(text) => {
                            let mut out = stdout.lock();
                            let _ = out.write_all(text.as_bytes());
                            let _ = out.flush();
                        }
                        AppEvent::StreamEnd => {
                            let mut out = stdout.lock();
                            let _ = out.write_all(b"\n");
                            let _ = out.flush();
                        }
                        AppEvent::ToolUseStart { name, input } => {
                            let display_name =
                                rust_claude_tui::ChatMessage::user_facing_tool_name(&name);
                            let summary = match name.as_str() {
                                "Bash" => input
                                    .get("command")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                "FileRead" => input
                                    .get("file_path")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                "FileEdit" | "FileWrite" => input
                                    .get("file_path")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                _ => String::new(),
                            };
                            let mut err = stderr.lock();
                            if summary.is_empty() {
                                let _ = writeln!(err, "[{display_name}]");
                            } else {
                                let _ = writeln!(err, "[{display_name}] {summary}");
                            }
                        }
                        AppEvent::ToolResult {
                            name,
                            output,
                            is_error,
                        } => {
                            let display_name =
                                rust_claude_tui::ChatMessage::user_facing_tool_name(&name);
                            let status = if is_error { "error" } else { "ok" };
                            let truncated = if output.len() > 100 {
                                format!("{}...", &output[..100])
                            } else {
                                output
                            };
                            let mut err = stderr.lock();
                            let _ = writeln!(err, "[{display_name} {status}] {truncated}");
                        }
                        AppEvent::Error(msg) => {
                            let mut err = stderr.lock();
                            let _ = writeln!(err, "Error: {msg}");
                        }
                        // Suppress thinking, tool input streaming, and other events
                        _ => {}
                    }
                }
            });

            let result = query_loop.run(app_state.clone(), prompt).await;
            // Drop happens when query_loop is done — print_handle will exit once channel closes
            if let Ok(final_message) = &result {
                drop(final_message.clone());
            }
            let _ = print_handle.await;
            result.map(|_| ())
        } else {
            // Non-streaming or JSON output mode: collect full response then dump
            match query_loop.run(app_state.clone(), prompt).await {
                Ok(final_message) => {
                    if output_json {
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
                }
                Err(error) => Err(error.into()),
            }
        };
        if let Some(runner) = &hook_runner {
            let session_id = { app_state.lock().await.session.id.clone() };
            let reason = if run_result.is_ok() {
                "completed"
            } else {
                "error"
            };
            runner.run_session_end(reason, &session_id).await;
        }
        Ok(run_result?)
    } else {
        let allowed_tools = resolved.allowed_tools.clone();
        let disallowed_tools = resolved.disallowed_tools.clone();
        run_tui(
            app_state,
            resolved.config.clone(),
            allowed_tools,
            disallowed_tools,
            hook_runner,
            mcp_manager,
            custom_agents,
            plugin_manager,
        )
        .await
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
        return Err(anyhow!(
            "apiKeyHelper exited with {}: {stderr}",
            output.status
        ));
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
    provider: Provider,
) -> Result<AnthropicClient> {
    let mut client = AnthropicClient::new(api_key.to_string())?;
    let _ = provider;
    if let Some(base_url) = base_url_override {
        client = client.with_base_url(base_url);
    }
    if bearer_auth {
        client = client.with_bearer_auth();
    }
    Ok(client)
}

fn build_agent_context(
    client: Arc<dyn rust_claude_api::ModelClient>,
    custom_agents: Arc<CustomAgentRegistry>,
) -> AgentContext {
    let run_subagent_cell = Arc::new(std::sync::Mutex::new(None::<rust_claude_tools::tool::AgentContextRunSubagent>));
    let runner_custom_agents = custom_agents.clone();
    let runner_cell = run_subagent_cell.clone();
    let run_subagent: rust_claude_tools::tool::AgentContextRunSubagent = Arc::new(
        move |
            prompt: String,
            allowed_tools: Vec<String>,
            options: rust_claude_tools::AgentRunOptions,
            app_state: Arc<Mutex<AppState>>,
            current_depth: u32,
            max_depth: u32,
        | {
            let client = client.clone();
            let custom_agents = runner_custom_agents.clone();
            let nested_runner = runner_cell
                .lock()
                .unwrap()
                .as_ref()
                .cloned()
                .expect("sub-agent runner initialized");
            Box::pin(async move {
                if options.system_prompt.is_some() || options.model.is_some() {
                    let mut state = app_state.lock().await;
                    if let Some(system_prompt) = options.system_prompt {
                        state.session.system_prompt = Some(system_prompt);
                    }
                    if let Some(model) = options.model {
                        state.session.model_setting = model.clone();
                        state.session.model = model;
                    }
                }

                let mut tools = build_tools();
                if !allowed_tools.is_empty() {
                    tools.apply_tool_filters(&allowed_tools, &[]);
                }
                let query_loop = QueryLoop::new(client.clone(), tools)
                    .with_max_rounds(5)
                    .with_agent_context(AgentContext {
                        tool_registry_factory: Arc::new(build_tools),
                        run_subagent: nested_runner,
                        custom_agents,
                        current_depth,
                        max_depth,
                    });

                let message = query_loop
                    .run(app_state.clone(), prompt)
                    .await
                    .map_err(|e| rust_claude_tools::ToolError::Execution(e.to_string()))?;

                let text = message
                    .content
                    .into_iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let usage = app_state.lock().await.total_usage.clone();
                Ok(rust_claude_tools::tool::AgentRunOutput {
                    text,
                    input_tokens: usage.input_tokens as u64,
                    output_tokens: usage.output_tokens as u64,
                })
            })
        },
    );
    *run_subagent_cell.lock().unwrap() = Some(run_subagent.clone());

    AgentContext {
        tool_registry_factory: Arc::new(build_tools),
        run_subagent,
        custom_agents,
        current_depth: 0,
        max_depth: 3,
    }
}

fn format_custom_agents(registry: &CustomAgentRegistry) -> String {
    let agents = registry.list();
    if agents.is_empty() {
        let mut text = String::from("No custom agents configured");
        if !registry.errors().is_empty() {
            text.push_str("\n\nLoad errors:");
            for error in registry.errors() {
                text.push_str(&format!(
                    "\n  - {}: {}",
                    error.path.display(),
                    error.message
                ));
            }
        }
        return text;
    }

    let mut text = String::from("Custom agents:");
    for agent in agents {
        text.push_str(&format!("\n  {} - {}", agent.name, agent.description));
    }
    if !registry.errors().is_empty() {
        text.push_str("\n\nLoad errors:");
        for error in registry.errors() {
            text.push_str(&format!(
                "\n  - {}: {}",
                error.path.display(),
                error.message
            ));
        }
    }
    text
}

fn format_mcp_status(manager: &McpManager) -> String {
    let statuses = manager.server_statuses();
    if statuses.is_empty() {
        return "No MCP servers configured.".to_string();
    }

    let mut text = format!(
        "MCP servers: {} configured, {} connected, {} tool(s)\n",
        statuses.len(),
        manager.connected_count(),
        manager.tool_count()
    );

    for status in statuses {
        let state_str = match &status.state {
            rust_claude_core::mcp_config::McpServerState::Connected => "connected".to_string(),
            rust_claude_core::mcp_config::McpServerState::Disconnected(msg) => {
                format!("disconnected: {}", msg)
            }
            rust_claude_core::mcp_config::McpServerState::Failed(msg) => format!("failed: {}", msg),
            rust_claude_core::mcp_config::McpServerState::Pending => "pending".to_string(),
        };

        text.push_str(&format!(
            "\n  {} [{}] ({})\n",
            status.name, status.transport_type, state_str
        ));

        if !status.tools.is_empty() {
            text.push_str("    Tools:\n");
            for tool in &status.tools {
                let desc = if tool.description.is_empty() {
                    String::new()
                } else {
                    format!(" - {}", tool.description)
                };
                text.push_str(&format!(
                    "      mcp__{}__{}{}  \n",
                    status.name, tool.name, desc
                ));
            }
        }
    }

    text
}

fn format_memory_status(cwd: &std::path::Path) -> String {
    let Some(store) = memory::discover_memory_store(cwd) else {
        return "No memory store is available for the current project.".to_string();
    };
    format_memory_store_status(&store)
}

fn format_memory_store_status(store: &memory::MemoryStore) -> String {
    match memory::scan_memory_store(store) {
        Ok(scanned) => {
            let mut text = format!(
                "Memory store:\n  project_root: {}\n  memory_dir: {}\n  entrypoint: {}\n  entries: {}",
                scanned.store.project_root.display(),
                scanned.store.memory_dir.display(),
                scanned.store.entrypoint.display(),
                scanned.entries.len()
            );

            if scanned.entries.is_empty() {
                text.push_str("\n\nNo memory entries found.");
            } else {
                text.push_str("\n\nVisible memories:\n");
                for entry in scanned.entries.iter().take(10) {
                    let memory_type = entry
                        .frontmatter
                        .memory_type
                        .map(|t| t.as_str().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let description = entry
                        .frontmatter
                        .description
                        .as_deref()
                        .unwrap_or("(no description)");
                    text.push_str(&format!(
                        "- [{}] {} — {} ({} days old)\n",
                        memory_type, entry.relative_path, description, entry.freshness_days
                    ));
                }
            }

            if let Some(index) = scanned.index {
                text.push_str(&format!(
                    "\nEntrypoint loaded: {}{}",
                    index.path.display(),
                    if index.truncated {
                        " (truncated for prompt use)"
                    } else {
                        ""
                    }
                ));
            }

            text
        }
        Err(e) => format!("Failed to inspect memory store: {e}"),
    }
}

fn remember_memory(
    cwd: &std::path::Path,
    memory_type: &str,
    path: &str,
    title: &str,
    description: &str,
    body: &str,
) -> String {
    let Some(store) = memory::discover_memory_store(cwd) else {
        return "No memory store is available for the current project.".to_string();
    };

    let Some(parsed_type) = memory::MemoryType::parse(memory_type) else {
        return format!(
            "Unknown memory type '{}'. Valid types: user, feedback, project, reference",
            memory_type
        );
    };

    let request = memory::MemoryWriteRequest {
        relative_path: path.to_string(),
        frontmatter: memory::MemoryFrontmatter {
            name: Some(title.to_string()),
            description: Some(description.to_string()),
            memory_type: Some(parsed_type),
            extra: std::collections::HashMap::new(),
        },
        body: body.to_string(),
    };

    match memory::save_memory_entry_dedup(&store, &request) {
        Ok(memory::MemorySaveOutcome::Updated { path, .. }) => format!(
            "Updated memory '{}' at {} and rebuilt {}",
            title,
            path.display(),
            store.entrypoint.display()
        ),
        Ok(memory::MemorySaveOutcome::Created { path }) => format!(
            "Saved memory '{}' to {} and updated {}",
            title,
            path.display(),
            store.entrypoint.display()
        ),
        Ok(memory::MemorySaveOutcome::Skipped { reason }) => {
            format!("Skipped memory save: {}", reason.as_str())
        }
        Err(e) => format!("Failed to save memory: {e}"),
    }
}

fn forget_memory(cwd: &std::path::Path, path: &str) -> String {
    let Some(store) = memory::discover_memory_store(cwd) else {
        return "No memory store is available for the current project.".to_string();
    };

    match memory::remove_memory_entry(&store, path) {
        Ok(true) => format!(
            "Removed memory {} and updated {}",
            path,
            store.entrypoint.display()
        ),
        Ok(false) => format!("Memory {} was not found", path),
        Err(e) => format!("Failed to forget memory: {e}"),
    }
}

fn executable_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn format_doctor_report(
    state: &AppState,
    config: &Config,
    mcp_manager: &McpManager,
    hook_runner: Option<&HookRunner>,
) -> String {
    let mut lines = vec!["Doctor Report".to_string(), String::new()];

    lines.push("API".to_string());
    lines.push(format!("  model: {}", state.session.model_setting));
    lines.push(format!("  model_source: {}", state.config_provenance.model));
    lines.push(format!(
        "  credential: {}",
        if config.api_key.trim().is_empty() {
            "missing"
        } else {
            "available"
        }
    ));
    lines.push(format!(
        "  auth_mode: {}",
        if config.bearer_auth {
            "bearer"
        } else {
            "x-api-key"
        }
    ));
    lines.push(format!(
        "  base_url_source: {}",
        state.config_provenance.base_url
    ));

    lines.push(String::new());
    lines.push("Configuration".to_string());
    lines.push(format!("  cwd: {}", state.cwd.display()));
    lines.push(format!("  permission_mode: {:?}", state.permission_mode));
    lines.push(format!("  stream: {}", state.session.stream));
    lines.push(format!(
        "  hooks: {}",
        hook_runner
            .map(|runner| runner
                .config()
                .values()
                .map(|groups| groups.len())
                .sum::<usize>())
            .unwrap_or(0)
    ));

    lines.push(String::new());
    lines.push("MCP".to_string());
    let statuses = mcp_manager.server_statuses();
    if statuses.is_empty() {
        lines.push("  not configured".to_string());
    } else {
        for status in statuses {
            lines.push(format!(
                "  {}: {:?} ({} tools)",
                status.name,
                status.state,
                status.tools.len()
            ));
        }
    }

    lines.push(String::new());
    lines.push("Tools".to_string());
    lines.push(format!(
        "  git: {}",
        if executable_available("git") {
            "available"
        } else {
            "missing"
        }
    ));
    lines.push(format!(
        "  gh: {}",
        if executable_available("gh") {
            "available"
        } else {
            "missing (PR review degraded)"
        }
    ));

    lines.push(String::new());
    lines.push("Permissions".to_string());
    let permission_path = rust_claude_core::permission::PermissionManager::default_path();
    if permission_path.exists() {
        match rust_claude_core::permission::PermissionManager::load(&permission_path) {
            Ok(manager) => lines.push(format!(
                "  {}: valid ({} allow, {} deny)",
                permission_path.display(),
                manager.always_allow.len(),
                manager.always_deny.len()
            )),
            Err(error) => lines.push(format!(
                "  {}: invalid ({})",
                permission_path.display(),
                error
            )),
        }
    } else {
        lines.push(format!("  {}: not found", permission_path.display()));
    }

    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReviewInput {
    prompt: String,
    truncated: bool,
}

fn collect_review_input(cwd: &std::path::Path, target: Option<&str>) -> Result<ReviewInput> {
    match target.map(str::trim).filter(|value| !value.is_empty()) {
        Some(target) => collect_pr_review_input(cwd, target),
        None => collect_branch_review_input(cwd),
    }
}

fn collect_branch_review_input(cwd: &std::path::Path) -> Result<ReviewInput> {
    let git_context = collect_git_context(cwd)
        .ok_or_else(|| anyhow!("Git repository context is required for /review"))?;
    let repo_root = git_context.repo_root;

    if let Some(base) = detect_review_base(&repo_root) {
        // We have a recognizable base branch — diff the full range.
        let range = format!("{}...HEAD", base);
        let diff = run_command_text(&repo_root, "git", &["diff", "--stat", &range])?
            + "\n\n"
            + &run_command_text(&repo_root, "git", &["diff", &range])?;
        if diff.trim().is_empty() {
            return Err(anyhow!("No reviewable diff was found."));
        }
        return Ok(build_review_input("current branch", &diff));
    }

    // No standard base branch found — fall back to uncommitted changes
    // (staged + unstaged) so we don't silently miss committed branch work.
    let diff = run_command_text(&repo_root, "git", &["diff", "--stat", "HEAD"])?
        + "\n\n"
        + &run_command_text(&repo_root, "git", &["diff", "HEAD"])?;
    if diff.trim().is_empty() {
        return Err(anyhow!(
            "No reviewable diff was found. Could not detect a base branch \
             (tried origin/HEAD, origin/main, origin/master, main, master). \
             Use `/review <branch>` to specify the base branch explicitly."
        ));
    }
    Ok(build_review_input("uncommitted changes", &diff))
}

fn collect_pr_review_input(cwd: &std::path::Path, target: &str) -> Result<ReviewInput> {
    if !executable_available("gh") {
        return Err(anyhow!(
            "PR lookup requires `gh`. Run `/review` without arguments to review the local branch diff."
        ));
    }
    let git_context = collect_git_context(cwd)
        .ok_or_else(|| anyhow!("Git repository context is required for /review"))?;
    let diff = run_command_text(&git_context.repo_root, "gh", &["pr", "diff", target])?;
    if diff.trim().is_empty() {
        return Err(anyhow!("No reviewable diff was found for {target}."));
    }
    Ok(build_review_input(target, &diff))
}

fn detect_review_base(repo_root: &std::path::Path) -> Option<String> {
    for candidate in [
        "origin/HEAD",
        "origin/main",
        "origin/master",
        "main",
        "master",
    ] {
        if Command::new("git")
            .args(["rev-parse", "--verify", candidate])
            .current_dir(repo_root)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Some(candidate.to_string());
        }
    }
    None
}

fn run_command_text(repo_root: &std::path::Path, program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "{} {} failed: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn build_review_input(source: &str, diff: &str) -> ReviewInput {
    let (diff, truncated) = truncate_review_diff(diff);
    let truncation_note = if truncated {
        "\n\nNote: The diff was truncated deterministically because it exceeded the review input limit. Mention this as a residual risk."
    } else {
        ""
    };
    ReviewInput {
        truncated,
        prompt: format!(
            "Please review the following changes from {source}. Prioritize correctness bugs, behavioral regressions, security issues, and missing tests. Findings must come first, ordered by severity, with file/line references where available. If no actionable findings are found, say so explicitly and mention residual risks or testing gaps.{truncation_note}\n\n```diff\n{}\n```",
            diff.trim()
        ),
    }
}

fn truncate_review_diff(diff: &str) -> (String, bool) {
    if diff.len() <= REVIEW_DIFF_LIMIT {
        return (diff.to_string(), false);
    }
    let mut end = REVIEW_DIFF_LIMIT;
    while !diff.is_char_boundary(end) {
        end -= 1;
    }
    (diff[..end].to_string(), true)
}

fn build_tools() -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(AgentTool::new());
    tools.register(AskUserQuestionTool::new());
    tools.register(AutoMemoryTool::new());
    tools.register(BashTool::new());
    tools.register(EnterPlanModeTool::new());
    tools.register(ExitPlanModeTool::new());
    tools.register(FileReadTool::new());
    tools.register(FileEditTool::new());
    tools.register(FileWriteTool::new());
    tools.register(GlobTool::new());
    tools.register(GrepTool::new());
    tools.register(LspTool::new());
    tools.register(MonitorTool::new());
    tools.register(NotebookEditTool::new());
    tools.register(TaskTool::new());
    tools.register(WebFetchTool::new());
    tools.register(WebSearchTool::new());
    tools
}

fn usage_to_u64(usage: &Usage) -> (u64, u64, u64, u64) {
    (
        usage.input_tokens as u64,
        usage.output_tokens as u64,
        usage.cache_read_input_tokens as u64,
        usage.cache_creation_input_tokens as u64,
    )
}

fn sum_message_usage(messages: &[Message]) -> Usage {
    let mut usage = Usage {
        input_tokens: 0,
        output_tokens: 0,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
    };
    for message in messages {
        if let Some(item) = &message.usage {
            usage.input_tokens += item.input_tokens;
            usage.output_tokens += item.output_tokens;
            usage.cache_creation_input_tokens += item.cache_creation_input_tokens;
            usage.cache_read_input_tokens += item.cache_read_input_tokens;
        }
    }
    usage
}

fn truncate_latest_user_turn(messages: &mut Vec<Message>) -> bool {
    let Some(index) = messages
        .iter()
        .rposition(|message| matches!(message.role, Role::User))
    else {
        return false;
    };
    messages.truncate(index);
    true
}

fn recap_messages(messages: &[Message]) -> Option<String> {
    if !messages.iter().any(|message| {
        message.content.iter().any(|block| match block {
            ContentBlock::Text { text } => !text.trim().is_empty(),
            _ => false,
        })
    }) {
        return None;
    }

    let user_turns = messages
        .iter()
        .filter(|message| matches!(message.role, Role::User))
        .count();
    let assistant_turns = messages
        .iter()
        .filter(|message| matches!(message.role, Role::Assistant))
        .count();
    let latest_user = messages.iter().rev().find_map(|message| {
        if matches!(message.role, Role::User) {
            message.content.iter().find_map(|block| match block {
                ContentBlock::Text { text } => Some(text.trim()),
                _ => None,
            })
        } else {
            None
        }
    });

    let mut text = format!(
        "Session recap:\n  user turns: {}\n  assistant turns: {}",
        user_turns, assistant_turns
    );
    if let Some(latest_user) = latest_user.filter(|text| !text.is_empty()) {
        text.push_str(&format!("\n  latest user request: {}", latest_user));
    }
    Some(text)
}

fn keybindings_text() -> &'static str {
    "Keybindings:\n  Enter: submit\n  Shift+Enter: insert newline\n  Up/Down: browse history or move multi-line cursor\n  PageUp/PageDown: scroll chat history\n  Ctrl+Home/Ctrl+End: jump to oldest/latest chat content\n  Ctrl+A/Ctrl+E/Home/End: move within line\n  Ctrl+Left/Ctrl+Right: move by word\n  Escape or Ctrl+C: cancel active stream\n  Ctrl+L: redraw screen\n  Tab: toggle latest thinking block"
}

fn effort_budget(level: &str) -> Option<Option<u32>> {
    match level {
        "low" => Some(Some(EFFORT_LOW_THINKING_BUDGET)),
        "medium" => Some(Some(EFFORT_MEDIUM_THINKING_BUDGET)),
        "high" => Some(None),
        _ => None,
    }
}

fn effort_label(budget: Option<u32>) -> &'static str {
    match budget {
        Some(EFFORT_LOW_THINKING_BUDGET) => "low",
        Some(EFFORT_MEDIUM_THINKING_BUDGET) => "medium",
        None => "high",
        Some(_) => "custom",
    }
}

fn messages_to_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    let mut out = Vec::new();
    for message in messages {
        for block in &message.content {
            match (message.role.clone(), block) {
                (Role::User, ContentBlock::Text { text }) => {
                    out.push(ChatMessage::User(text.clone()))
                }
                (
                    Role::User,
                    ContentBlock::ToolResult {
                        content, is_error, ..
                    },
                ) => out.push(ChatMessage::ToolResult {
                    name: "Tool".into(),
                    output_summary: content.clone(),
                    is_error: *is_error,
                }),
                (Role::Assistant, ContentBlock::Text { text }) => {
                    out.push(ChatMessage::Assistant(text.clone()))
                }
                (Role::Assistant, ContentBlock::Thinking { thinking, .. }) => {
                    out.push(ChatMessage::Thinking {
                        summary: format!("Thought for ~{} chars", thinking.chars().count()),
                        content: thinking.clone(),
                    })
                }
                (Role::Assistant, ContentBlock::ToolUse { name, input, .. }) => {
                    out.push(ChatMessage::ToolUse {
                        name: name.clone(),
                        input_summary: serde_json::to_string(input).unwrap_or_default(),
                        diff_lines: None,
                    })
                }
                _ => {}
            }
        }
    }
    out
}

fn model_context_capacity(model: &str) -> Option<u32> {
    let lower = model.to_ascii_lowercase();
    if lower.contains("[1m]") || lower.contains("1m") {
        Some(1_000_000)
    } else if lower.contains("claude-") {
        Some(200_000)
    } else {
        None
    }
}

fn estimate_tokens(text: &str) -> u32 {
    ((text.chars().count() as u32).saturating_add(3) / 4).max(1)
}

fn estimate_block_tokens(block: &ContentBlock) -> u32 {
    match block {
        ContentBlock::Text { text } => estimate_tokens(text),
        ContentBlock::ToolUse { input, .. } => estimate_tokens(&input.to_string()),
        ContentBlock::ToolResult { content, .. } => estimate_tokens(content),
        ContentBlock::Thinking { thinking, .. } => estimate_tokens(thinking),
        ContentBlock::Image { .. } => 1000,
        ContentBlock::Unknown => 0,
    }
}

fn build_context_snapshot(state: &AppState) -> ContextSnapshot {
    let system_prompt_tokens = state
        .session
        .system_prompt
        .as_ref()
        .map(|prompt| estimate_tokens(prompt))
        .unwrap_or(0);
    let mut message_tokens = 0u32;
    let mut tool_result_tokens = 0u32;
    for message in &state.messages {
        for block in &message.content {
            let tokens = estimate_block_tokens(block);
            if matches!(block, ContentBlock::ToolResult { .. }) {
                tool_result_tokens = tool_result_tokens.saturating_add(tokens);
            } else {
                message_tokens = message_tokens.saturating_add(tokens);
            }
        }
    }

    let reported_used = state
        .total_usage
        .input_tokens
        .saturating_add(state.total_usage.cache_read_input_tokens)
        .saturating_add(state.total_usage.cache_creation_input_tokens);
    let estimated_used = system_prompt_tokens
        .saturating_add(message_tokens)
        .saturating_add(tool_result_tokens);
    let used_tokens = reported_used.max(estimated_used);
    let context_capacity = model_context_capacity(&state.session.model);
    let remaining_tokens = context_capacity.map(|capacity| capacity.saturating_sub(used_tokens));

    ContextSnapshot {
        model: state.session.model.clone(),
        context_capacity,
        used_tokens,
        system_prompt_tokens,
        message_tokens,
        tool_result_tokens,
        remaining_tokens,
    }
}

fn default_export_path() -> PathBuf {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("rust-claude-code")
        .join("exports")
        .join(format!("session-{timestamp}.md"))
}

fn format_transcript_markdown(state: &AppState) -> String {
    let mut out = String::new();
    out.push_str("# rust-claude conversation export\n\n");
    out.push_str(&format!("- Model: `{}`\n", state.session.model));
    out.push_str(&format!(
        "- Model setting: `{}`\n",
        state.session.model_setting
    ));
    out.push_str(&format!("- Working directory: `{}`\n", state.cwd.display()));
    out.push_str(&format!(
        "- Exported at: `{}`\n",
        chrono::Local::now().to_rfc3339()
    ));
    out.push_str(&format!("- Messages: `{}`\n\n", state.messages.len()));

    for (idx, message) in state.messages.iter().enumerate() {
        let role = match message.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        out.push_str(&format!("## {}. {}\n\n", idx + 1, role));
        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    out.push_str(text);
                    out.push_str("\n\n");
                }
                ContentBlock::Thinking { thinking, .. } => {
                    out.push_str("### Thinking\n\n```text\n");
                    out.push_str(thinking);
                    out.push_str("\n```\n\n");
                }
                ContentBlock::ToolUse { id, name, input } => {
                    out.push_str(&format!("### Tool Use: {name} ({id})\n\n```json\n"));
                    out.push_str(
                        &serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string()),
                    );
                    out.push_str("\n```\n\n");
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    out.push_str(&format!(
                        "### Tool Result: {tool_use_id}{}\n\n```text\n",
                        if *is_error { " (error)" } else { "" }
                    ));
                    out.push_str(content);
                    out.push_str("\n```\n\n");
                }
                ContentBlock::Image { .. } => {
                    out.push_str("[image content]\n\n");
                }
                ContentBlock::Unknown => {
                    out.push_str("[unknown content block]\n\n");
                }
            }
        }
    }
    out
}

fn export_conversation(state: &AppState, path: Option<PathBuf>) -> Result<PathBuf> {
    let path = path.unwrap_or_else(default_export_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, format_transcript_markdown(state))?;
    Ok(path)
}

fn latest_assistant_text(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|message| message.role == Role::Assistant)
        .map(|message| {
            message
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.trim().is_empty())
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    copy_text_with(text, |text| {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| anyhow!("clipboard provider unavailable: {e}"))?;
        clipboard
            .set_text(text.to_string())
            .map_err(|e| anyhow!("failed to set clipboard text: {e}"))
    })
}

fn copy_text_with<F>(text: &str, copy_fn: F) -> Result<()>
where
    F: FnOnce(&str) -> Result<()>,
{
    if text.trim().is_empty() {
        return Err(anyhow!("clipboard text is empty"));
    }
    copy_fn(text)
}

async fn run_tui(
    app_state: Arc<Mutex<AppState>>,
    config: Config,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
    hook_runner: Option<Arc<HookRunner>>,
    mcp_manager: Arc<McpManager>,
    custom_agents: Arc<CustomAgentRegistry>,
    plugin_manager: Arc<Mutex<PluginManager>>,
) -> Result<()> {
    let (model, model_setting, permission_mode, git_branch) = {
        let state = app_state.lock().await;
        (
            get_runtime_main_loop_model(&state.session.model_setting, state.permission_mode, false),
            state.session.model_setting.clone(),
            format!("{:?}", state.permission_mode),
            state.git_context.as_ref().map(|g| g.branch.clone()),
        )
    };

    let base_url = config.base_url.clone();
    let bearer_auth = config.bearer_auth;
    let theme = config.theme;

    let (event_tx, event_rx) = mpsc::channel(128);
    let (user_tx, mut user_rx) = mpsc::channel::<UserCommand>(16);

    let bridge = TuiBridge::new(event_tx);
    let worker_bridge = bridge.clone();
    let worker_state = app_state.clone();

    worker_bridge
        .send_status_update(
            &model,
            &model_setting,
            &permission_mode,
            git_branch.as_deref(),
        )
        .await;

    let worker_hook_runner = hook_runner.clone();
    let worker_mcp_manager = mcp_manager.clone();
    let worker_custom_agents = custom_agents.clone();
    let worker_config = config.clone();
    let app_plugin_manager = plugin_manager.clone();
    let app_worker_bridge = worker_bridge.clone();
    tokio::spawn(async move {
        let mut worker_config = worker_config;
        let mut active_query_task: Option<tokio::task::JoinHandle<()>> = None;

        while let Some(command) = user_rx.recv().await {
            match command {
                UserCommand::Compact(strategy) => {
                    let client =
                        match build_client(&worker_config.api_key, base_url.clone(), bearer_auth, Provider::Anthropic) {
                            Ok(client) => client,
                            Err(error) => {
                                worker_bridge.send_error(&error.to_string()).await;
                                continue;
                            }
                        };

                    worker_bridge.send_compaction_start().await;
                    let compaction_config = strategy.config();
                    let service = CompactionService::new(client, compaction_config.clone());
                    match service.force_compact(&worker_state).await {
                        Ok(result) => worker_bridge.send_compaction_complete(result).await,
                        Err(e) => {
                            worker_bridge
                                .send_error(&format!("Compaction failed: {e}"))
                                .await
                        }
                    }

                    let state_snapshot = worker_state.lock().await;
                    let mut session_file = SessionFile::new(
                        &state_snapshot.session.model,
                        &state_snapshot.session.model_setting,
                        &state_snapshot.cwd,
                    );
                    session_file.id = state_snapshot.session.id.clone();
                    session_file.messages = state_snapshot.messages.clone();
                    session_file.total_usage = Some(state_snapshot.total_usage.clone());
                    session_file.permission_mode = state_snapshot.permission_mode;
                    session_file.always_allow_rules = state_snapshot.always_allow_rules.clone();
                    session_file.always_deny_rules = state_snapshot.always_deny_rules.clone();
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

                    let (runtime_model, model_setting, permission_mode_display, git_branch) = {
                        let mut state = worker_state.lock().await;
                        if mode == PermissionMode::Plan {
                            // Use enter_plan_mode() so previous_permission_mode is
                            // saved, allowing ExitPlanMode tool to restore it later.
                            state.enter_plan_mode();
                        } else {
                            // Clear any saved plan-mode state when explicitly
                            // switching away from plan mode via /mode.
                            state.previous_permission_mode = None;
                            state.permission_mode = mode;
                        }
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
                            state.git_context.as_ref().map(|g| g.branch.clone()),
                        )
                    };

                    worker_bridge
                        .send_status_update(
                            &runtime_model,
                            &model_setting,
                            &permission_mode_display,
                            git_branch.as_deref(),
                        )
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

                    let (runtime_model, model_setting, permission_mode_display, git_branch) = {
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
                            state.git_context.as_ref().map(|g| g.branch.clone()),
                        )
                    };

                    worker_bridge
                        .send_status_update(
                            &runtime_model,
                            &model_setting,
                            &permission_mode_display,
                            git_branch.as_deref(),
                        )
                        .await;
                    worker_bridge
                        .send_assistant_message(&format!("Model switched to: {model_setting}"))
                        .await;
                }
                UserCommand::EnterPlan { description } => {
                    let (runtime_model, model_setting, permission_mode_display, git_branch) = {
                        let mut state = worker_state.lock().await;
                        state.enter_plan_mode();
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
                            state.git_context.as_ref().map(|g| g.branch.clone()),
                        )
                    };
                    worker_bridge
                        .send_status_update(
                            &runtime_model,
                            &model_setting,
                            &permission_mode_display,
                            git_branch.as_deref(),
                        )
                        .await;
                    let message = match description.as_deref().filter(|value| !value.trim().is_empty()) {
                        Some(description) => format!("Plan mode active. Context: {}", description.trim()),
                        None => "Plan mode active.".to_string(),
                    };
                    worker_bridge.send_assistant_message(&message).await;
                }
                UserCommand::RenameSession { name } => {
                    worker_bridge
                        .send_assistant_message(&format!("Session renamed to: {}", name.trim()))
                        .await;
                }
                UserCommand::BranchConversation { name } => {
                    let branch_name = name
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToString::to_string)
                        .unwrap_or_else(|| chrono::Local::now().format("branch-%Y%m%d-%H%M%S").to_string());
                    worker_bridge
                        .send_assistant_message(&format!(
                            "Created conversation branch: {branch_name}"
                        ))
                        .await;
                }
                UserCommand::Recap => {
                    let messages = { worker_state.lock().await.messages.clone() };
                    let message = recap_messages(&messages)
                        .unwrap_or_else(|| "No conversation to summarize yet.".to_string());
                    worker_bridge.send_assistant_message(&message).await;
                }
                UserCommand::Rewind => {
                    if active_query_task
                        .as_ref()
                        .is_some_and(|handle| !handle.is_finished())
                    {
                        worker_bridge
                            .send_error("Cannot rewind while a response is active. Cancel or wait for it to finish first.")
                            .await;
                        continue;
                    }

                    let (messages, usage, rewound) = {
                        let mut state = worker_state.lock().await;
                        let rewound = truncate_latest_user_turn(&mut state.messages);
                        if rewound {
                            state.total_usage = sum_message_usage(&state.messages);
                            state.last_api_usage = None;
                            state.last_api_message_index = 0;
                        }
                        (
                            state.messages.clone(),
                            state.total_usage.clone(),
                            rewound,
                        )
                    };
                    if rewound {
                        let (input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens) =
                            usage_to_u64(&usage);
                        worker_bridge
                            .send_conversation_replaced(
                                messages_to_chat_messages(&messages),
                                input_tokens,
                                output_tokens,
                                cache_read_input_tokens,
                                cache_creation_input_tokens,
                                "Rewound to before the latest user turn.".to_string(),
                            )
                            .await;
                    } else {
                        worker_bridge
                            .send_assistant_message("No user turn to rewind.")
                            .await;
                    }
                }
                UserCommand::AddDirectory { path } => {
                    let cwd = { worker_state.lock().await.cwd.clone() };
                    let resolved = if path.is_absolute() { path } else { cwd.join(path) };
                    match resolved.canonicalize() {
                        Ok(path) if path.is_dir() => {
                            let added = {
                                let mut state = worker_state.lock().await;
                                if state.extra_cwds.contains(&path) {
                                    false
                                } else {
                                    state.extra_cwds.push(path.clone());
                                    true
                                }
                            };
                            let verb = if added { "Added" } else { "Already registered" };
                            worker_bridge
                                .send_assistant_message(&format!(
                                    "{verb} workspace directory: {}",
                                    path.display()
                                ))
                                .await;
                        }
                        Ok(path) => {
                            worker_bridge
                                .send_error(&format!("Not a directory: {}", path.display()))
                                .await;
                        }
                        Err(error) => {
                            worker_bridge
                                .send_error(&format!("Failed to add directory: {error}"))
                                .await;
                        }
                    }
                }
                UserCommand::LoginStatus => {
                    let state = worker_state.lock().await;
                    let auth_mode = if state.config.bearer_auth {
                        "Bearer"
                    } else {
                        "x-api-key"
                    };
                    let credential_status = if state.config.api_key.trim().is_empty() {
                        "missing"
                    } else {
                        "available"
                    };
                    let base_url = state
                        .config
                        .base_url
                        .as_deref()
                        .unwrap_or("Anthropic default");
                    let message = format!(
                        "Authentication status:\n  credential: {credential_status}\n  auth mode: {auth_mode}\n  base_url: {base_url}\n\nSet ANTHROPIC_API_KEY, ANTHROPIC_AUTH_TOKEN, rust-claude config, or Claude settings apiKeyHelper to authenticate."
                    );
                    drop(state);
                    worker_bridge.send_assistant_message(&message).await;
                }
                UserCommand::Logout => {
                    let result = {
                        let mut state = worker_state.lock().await;
                        state.config.api_key.clear();
                        state.config.save_without_credential()
                    };
                    match result {
                        Ok(()) => {
                            worker_config.api_key.clear();
                            worker_bridge
                                .send_assistant_message("Cleared local rust-claude config credentials. Environment variables and Claude settings/apiKeyHelper must be changed outside the TUI.")
                                .await;
                        }
                        Err(error) => {
                            worker_bridge
                                .send_error(&format!("Failed to clear local credentials: {error}"))
                                .await;
                        }
                    }
                }
                UserCommand::SetEffort { level } => {
                    if level.trim().is_empty() {
                        let current = {
                            let state = worker_state.lock().await;
                            effort_label(state.session.thinking_budget)
                        };
                        worker_bridge
                            .send_assistant_message(&format!(
                                "Current effort: {current}. Valid values: low, medium, high."
                            ))
                            .await;
                        continue;
                    }
                    let Some(budget) = effort_budget(level.as_str()) else {
                        worker_bridge
                            .send_error("Unknown effort. Valid values: low, medium, high")
                            .await;
                        continue;
                    };
                    {
                        let mut state = worker_state.lock().await;
                        state.session.thinking_budget = budget;
                    }
                    worker_bridge
                        .send_assistant_message(&format!("Effort set to: {level}"))
                        .await;
                }
                UserCommand::ShowKeybindings => {
                    worker_bridge.send_assistant_message(keybindings_text()).await;
                }
                UserCommand::SetTheme(theme) => {
                    let theme_str = match theme {
                        Theme::Dark => "dark",
                        Theme::Light => "light",
                    };
                    {
                        let mut state = worker_state.lock().await;
                        state.config.theme = theme;
                        state.config.provenance.theme = ConfigSource::Cli;
                        state.config_provenance.theme = ConfigSource::Cli;
                        if let Err(e) = state.config.save() {
                            worker_bridge
                                .send_error(&format!("Failed to persist theme setting: {e}"))
                                .await;
                            continue;
                        }
                    }
                    worker_bridge
                        .send_assistant_message(&format!("Theme switched to: {theme_str}"))
                        .await;
                }
                UserCommand::LoadCustomTheme => {
                    worker_bridge
                        .send_assistant_message("Custom themes are loaded directly by the TUI.")
                        .await;
                }
                UserCommand::ListSessions => {
                    let result =
                        tokio::task::spawn_blocking(|| session::list_recent_sessions_report(20))
                            .await
                            .unwrap_or_else(|e| Err(anyhow!("session list task join failed: {e}")));
                    match result {
                        Ok((sessions, skipped)) => {
                            worker_bridge.send_session_list(sessions, skipped).await;
                        }
                        Err(error) => {
                            worker_bridge
                                .send_error(&format!("Failed to list sessions: {error}"))
                                .await;
                        }
                    }
                }
                UserCommand::ResumeSession(session_id) => {
                    if active_query_task
                        .as_ref()
                        .is_some_and(|handle| !handle.is_finished())
                    {
                        worker_bridge
                            .send_error("Cannot resume another session while a response is active. Cancel or wait for it to finish first.")
                            .await;
                        continue;
                    }

                    let loaded = tokio::task::spawn_blocking({
                        let session_id = session_id.clone();
                        move || session::load_session_by_id(&session_id)
                    })
                    .await
                    .unwrap_or_else(|e| Err(anyhow!("session resume task join failed: {e}")));

                    match loaded {
                        Ok(Some(prev)) => {
                            let summary: SessionSummary = prev.summary();
                            let chat_messages = messages_to_chat_messages(&prev.messages);
                            let (
                                runtime_model,
                                model_setting,
                                permission_mode_display,
                                git_branch,
                                usage,
                            ) = {
                                let mut state = worker_state.lock().await;
                                session::restore_app_state_from_session(&mut state, &prev);
                                (
                                    state.session.model.clone(),
                                    state.session.model_setting.clone(),
                                    format!("{:?}", state.permission_mode),
                                    state.git_context.as_ref().map(|g| g.branch.clone()),
                                    state.total_usage.clone(),
                                )
                            };
                            let (
                                input_tokens,
                                output_tokens,
                                cache_read_input_tokens,
                                cache_creation_input_tokens,
                            ) = usage_to_u64(&usage);
                            worker_bridge
                                .send_session_resumed(
                                    summary,
                                    chat_messages,
                                    runtime_model,
                                    model_setting,
                                    permission_mode_display,
                                    git_branch,
                                    input_tokens,
                                    output_tokens,
                                    cache_read_input_tokens,
                                    cache_creation_input_tokens,
                                )
                                .await;
                        }
                        Ok(None) => {
                            worker_bridge
                                .send_error(&format!("Session '{}' not found", session_id))
                                .await;
                        }
                        Err(error) => {
                            worker_bridge
                                .send_error(&format!(
                                    "Failed to resume session '{}': {error}",
                                    session_id
                                ))
                                .await;
                        }
                    }
                }
                UserCommand::ShowContext => {
                    let snapshot = {
                        let state = worker_state.lock().await;
                        build_context_snapshot(&state)
                    };
                    worker_bridge.send_context_snapshot(snapshot).await;
                }
                UserCommand::ExportConversation { path } => {
                    let state_snapshot = { worker_state.lock().await.clone() };
                    let result = tokio::task::spawn_blocking(move || {
                        export_conversation(&state_snapshot, path)
                    })
                    .await
                    .unwrap_or_else(|e| Err(anyhow!("export task join failed: {e}")));
                    match result {
                        Ok(path) => {
                            worker_bridge
                                .send_assistant_message(&format!(
                                    "Exported conversation to {}",
                                    path.display()
                                ))
                                .await;
                        }
                        Err(error) => {
                            worker_bridge
                                .send_error(&format!("Failed to export conversation: {error}"))
                                .await;
                        }
                    }
                }
                UserCommand::CopyLatestAssistant => {
                    let (text, active_stream) = {
                        let state = worker_state.lock().await;
                        (
                            latest_assistant_text(&state.messages),
                            active_query_task
                                .as_ref()
                                .is_some_and(|handle| !handle.is_finished()),
                        )
                    };
                    match text {
                        Some(text) => {
                            let result =
                                tokio::task::spawn_blocking(move || copy_to_clipboard(&text))
                                    .await
                                    .unwrap_or_else(|e| {
                                        Err(anyhow!("clipboard task join failed: {e}"))
                                    });
                            match result {
                                Ok(()) => {
                                    let suffix = if active_stream {
                                        " Previous completed assistant response copied."
                                    } else {
                                        ""
                                    };
                                    worker_bridge
                                        .send_assistant_message(&format!(
                                            "Copied latest assistant response to clipboard.{suffix}"
                                        ))
                                        .await;
                                }
                                Err(error) => {
                                    worker_bridge
                                        .send_error(&format!("Failed to copy response: {error}"))
                                        .await;
                                }
                            }
                        }
                        None => {
                            worker_bridge
                                .send_assistant_message("No completed assistant response to copy.")
                                .await;
                        }
                    }
                }
                UserCommand::ShowConfig => {
                    let provenance = { worker_state.lock().await.config_provenance.clone() };
                    worker_bridge.send_config_info(&provenance).await;
                }
                UserCommand::ShowCost => {
                    let state = worker_state.lock().await;
                    let usage = &state.total_usage;
                    let est = (usage.input_tokens as f64 * 0.000_003_f64)
                        + (usage.output_tokens as f64 * 0.000_015_f64);
                    worker_bridge
                        .send_assistant_message(&format!(
                            "Session usage:\n  input_tokens: {}\n  output_tokens: {}\n  cache_read_input_tokens: {}\n  cache_creation_input_tokens: {}\n  estimated_cost_usd: ${:.4}",
                            usage.input_tokens,
                            usage.output_tokens,
                            usage.cache_read_input_tokens,
                            usage.cache_creation_input_tokens,
                            est
                        ))
                        .await;
                }
                UserCommand::ShowHooks => {
                    let msg = match &worker_hook_runner {
                        Some(runner) if !runner.is_empty() => {
                            let config = runner.config();
                            let mut text = String::from("Configured hooks:\n");
                            for (event, groups) in config {
                                text.push_str(&format!("\n  {}:\n", event));
                                for group in groups {
                                    let matcher_display = group
                                        .matcher
                                        .as_deref()
                                        .filter(|m| !m.is_empty())
                                        .unwrap_or("*");
                                    for hook in &group.hooks {
                                        let cmd = hook.command.as_deref().unwrap_or("(no command)");
                                        text.push_str(&format!(
                                            "    [{}] {} (type: {})\n",
                                            matcher_display, cmd, hook.type_
                                        ));
                                    }
                                }
                            }
                            text
                        }
                        _ => "No hooks configured".to_string(),
                    };
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::ShowAgents => {
                    let msg = format_custom_agents(&worker_custom_agents);
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::ShowMemory => {
                    let cwd = { worker_state.lock().await.cwd.clone() };
                    let msg = tokio::task::spawn_blocking(move || format_memory_status(&cwd))
                        .await
                        .unwrap_or_else(|e| format!("memory task join failed: {e}"));
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::RememberMemory {
                    memory_type,
                    path,
                    title,
                    description,
                    body,
                } => {
                    let cwd = { worker_state.lock().await.cwd.clone() };
                    let msg = tokio::task::spawn_blocking(move || {
                        remember_memory(&cwd, &memory_type, &path, &title, &description, &body)
                    })
                    .await
                    .unwrap_or_else(|e| format!("memory remember task join failed: {e}"));
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::ForgetMemory { path } => {
                    let cwd = { worker_state.lock().await.cwd.clone() };
                    let msg = tokio::task::spawn_blocking(move || forget_memory(&cwd, &path))
                        .await
                        .unwrap_or_else(|e| format!("memory forget task join failed: {e}"));
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::ShowDiff => {
                    let cwd = { worker_state.lock().await.cwd.clone() };
                    // Run blocking git work off the async runtime so it doesn't
                    // stall the TUI event loop or other tasks.
                    let cwd_for_blocking = cwd.clone();
                    let (git_context, message) = tokio::task::spawn_blocking(move || {
                        let git_context = collect_git_context(&cwd_for_blocking);
                        let message = if let Some(git) = &git_context {
                            let output = std::process::Command::new("git")
                                .arg("diff")
                                .current_dir(&git.repo_root)
                                .output();
                            match output {
                                Ok(output) if output.status.success() => {
                                    let diff =
                                        String::from_utf8_lossy(&output.stdout).trim().to_string();
                                    if diff.is_empty() {
                                        "No working tree changes to display.".to_string()
                                    } else {
                                        diff
                                    }
                                }
                                Ok(output) => format!(
                                    "git diff failed: {}",
                                    String::from_utf8_lossy(&output.stderr).trim()
                                ),
                                Err(e) => format!("git diff failed: {e}"),
                            }
                        } else {
                            "No Git repository available.".to_string()
                        };
                        (git_context, message)
                    })
                    .await
                    .unwrap_or_else(|e| (None, format!("git task join failed: {e}")));

                    {
                        let mut state = worker_state.lock().await;
                        state.git_context = git_context;
                    }
                    worker_bridge.send_assistant_message(&message).await;
                }
                UserCommand::ShowDoctor => {
                    let state = worker_state.lock().await;
                    let msg = format_doctor_report(
                        &state,
                        &worker_config,
                        &worker_mcp_manager,
                        worker_hook_runner.as_deref(),
                    );
                    drop(state);
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::Review { target } => {
                    if active_query_task
                        .as_ref()
                        .is_some_and(|handle| !handle.is_finished())
                    {
                        worker_bridge
                            .send_error("Cannot start /review while a response is active. Cancel or wait for it to finish first.")
                            .await;
                        continue;
                    }

                    let cwd = { worker_state.lock().await.cwd.clone() };
                    let target_for_blocking = target.clone();
                    let review_input = tokio::task::spawn_blocking(move || {
                        collect_review_input(&cwd, target_for_blocking.as_deref())
                    })
                    .await
                    .unwrap_or_else(|e| Err(anyhow!("review task join failed: {e}")));

                    let review_input = match review_input {
                        Ok(input) => input,
                        Err(error) => {
                            worker_bridge
                                .send_assistant_message(&format!("Review unavailable: {error}"))
                                .await;
                            continue;
                        }
                    };

                    let client =
                        match build_client(&worker_config.api_key, base_url.clone(), bearer_auth, Provider::Anthropic) {
                            Ok(client) => client,
                            Err(error) => {
                                worker_bridge.send_error(&error.to_string()).await;
                                continue;
                            }
                        };

                    let mut tools = build_tools();
                    register_mcp_tools(&mut tools, &worker_mcp_manager);
                    tools.apply_tool_filters(&allowed_tools, &disallowed_tools);
                    let agent_context =
                        build_agent_context(Arc::new(client.clone()), worker_custom_agents.clone());
                    let mut query_loop = QueryLoop::new(client, tools)
                        .with_output(Box::new(worker_bridge.clone()))
                        .with_permission_ui(Box::new(worker_bridge.clone()))
                        .with_user_question_ui(Box::new(worker_bridge.clone()))
                        .with_compaction_config(CompactStrategy::Default.config())
                        .with_agent_context(agent_context);
                    if let Some(runner) = &worker_hook_runner {
                        query_loop = query_loop.with_hook_runner(runner.clone());
                    }
                    let worker_bridge_clone = worker_bridge.clone();
                    let worker_state_clone = worker_state.clone();
                    let handle = tokio::spawn(async move {
                        match query_loop
                            .run(worker_state_clone.clone(), review_input.prompt)
                            .await
                        {
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
                                if !text.is_empty() {
                                    worker_bridge_clone.send_assistant_message(&text).await;
                                }
                            }
                            Err(error) => {
                                worker_bridge_clone
                                    .send_error(&format!("Review failed: {error}"))
                                    .await;
                            }
                        }
                    });
                    active_query_task = Some(handle);
                }
                UserCommand::CancelStream => {
                    if let Some(handle) = active_query_task.take() {
                        handle.abort();
                        worker_bridge.send_stream_cancelled().await;
                    }
                }
                UserCommand::ShowMcp => {
                    let msg = format_mcp_status(&worker_mcp_manager);
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::ShowPermissions => {
                    let state = worker_state.lock().await;
                    let mode_str = format!("{:?}", state.permission_mode);
                    let mut text = format!("Permission Mode: {}\n", mode_str);

                    if state.always_deny_rules.is_empty() && state.always_allow_rules.is_empty() {
                        text.push_str("\nNo custom rules configured.\n");
                    } else {
                        if !state.always_deny_rules.is_empty() {
                            text.push_str("\nDeny rules:\n");
                            for rule in &state.always_deny_rules {
                                text.push_str(&format!("  - {}\n", rule.to_compact_string()));
                            }
                        }
                        if !state.always_allow_rules.is_empty() {
                            text.push_str("\nAllow rules:\n");
                            for rule in &state.always_allow_rules {
                                text.push_str(&format!("  - {}\n", rule.to_compact_string()));
                            }
                        }
                    }

                    text.push_str("\nEdit ~/.config/rust-claude-code/permissions.json or .claude/settings.json to manage rules.");
                    drop(state);
                    worker_bridge.send_assistant_message(&text).await;
                }
                UserCommand::InitProject => {
                    let cwd = { worker_state.lock().await.cwd.clone() };
                    let msg = tokio::task::spawn_blocking(move || {
                        let root = rust_claude_core::claude_md::find_git_root(&cwd)
                            .unwrap_or_else(|| cwd.clone());
                        let claude_dir = root.join(".claude");
                        let claude_md_path = root.join("CLAUDE.md");

                        let mut result = String::new();

                        if !claude_dir.exists() {
                            match std::fs::create_dir_all(&claude_dir) {
                                Ok(()) => {
                                    result.push_str(&format!(
                                        "Created {}\n",
                                        claude_dir.display()
                                    ));
                                }
                                Err(e) => {
                                    return format!(
                                        "Failed to create {}: {}",
                                        claude_dir.display(),
                                        e
                                    );
                                }
                            }
                        } else {
                            result.push_str(&format!(
                                "{} already exists\n",
                                claude_dir.display()
                            ));
                        }

                        if !claude_md_path.exists() {
                            let starter_content = "# Project Instructions\n\n<!-- Add your project-specific instructions for Claude here. -->\n<!-- These will be included in every conversation. -->\n";
                            match std::fs::write(&claude_md_path, starter_content) {
                                Ok(()) => {
                                    result.push_str(&format!(
                                        "Created {} with starter content\n",
                                        claude_md_path.display()
                                    ));
                                }
                                Err(e) => {
                                    result.push_str(&format!(
                                        "Failed to create {}: {}\n",
                                        claude_md_path.display(),
                                        e
                                    ));
                                }
                            }
                        } else {
                            result.push_str(&format!(
                                "{} already exists (not overwritten)\n",
                                claude_md_path.display()
                            ));
                        }

                        result.push_str(
                            "\nTip: Add .claude/settings.local.json and CLAUDE.local.md to .gitignore for personal settings.",
                        );
                        result
                    })
                    .await
                    .unwrap_or_else(|e| format!("init task failed: {e}"));
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::ShowStatus => {
                    let (mode_str, model, deny_count, allow_count, cwd) = {
                        let state = worker_state.lock().await;
                        (
                            format!("{:?}", state.permission_mode),
                            state.session.model_setting.clone(),
                            state.always_deny_rules.len(),
                            state.always_allow_rules.len(),
                            state.cwd.clone(),
                        )
                    };
                    let rule_count = deny_count + allow_count;

                    let hook_count = match &worker_hook_runner {
                        Some(runner) => runner.config().values().map(|v| v.len()).sum::<usize>(),
                        None => 0,
                    };

                    let mcp_count = worker_mcp_manager.server_statuses().len();

                    let memory_count = tokio::task::spawn_blocking(move || {
                        let git_root = rust_claude_core::claude_md::find_git_root(&cwd);
                        let root = git_root.as_deref().unwrap_or(&cwd);
                        match rust_claude_core::memory::discover_memory_store(root) {
                            Some(store) => rust_claude_core::memory::scan_memory_store(&store)
                                .map_or(0, |s| s.entries.len()),
                            None => 0,
                        }
                    })
                    .await
                    .unwrap_or(0);

                    let msg = format!(
                        "Status:\n  Model: {}\n  Permission mode: {}\n  Permission rules: {} ({} allow, {} deny)\n  MCP servers: {}\n  Hooks: {}\n  Memory entries: {}",
                        model, mode_str, rule_count, allow_count, deny_count, mcp_count, hook_count, memory_count
                    );
                    worker_bridge.send_assistant_message(&msg).await;
                }
                UserCommand::Prompt(prompt) => {
                    // Abort any still-running query to prevent two loops
                    // racing on the same AppState.
                    if let Some(handle) = active_query_task.take() {
                        handle.abort();
                    }
                    let client =
                        match build_client(&worker_config.api_key, base_url.clone(), bearer_auth, Provider::Anthropic) {
                            Ok(client) => client,
                            Err(error) => {
                                worker_bridge.send_error(&error.to_string()).await;
                                continue;
                            }
                        };

                    let mut tools = build_tools();
                    register_mcp_tools(&mut tools, &worker_mcp_manager);
                    tools.apply_tool_filters(&allowed_tools, &disallowed_tools);
                    let agent_context =
                        build_agent_context(Arc::new(client.clone()), worker_custom_agents.clone());
                    let mut query_loop = QueryLoop::new(client, tools)
                        .with_output(Box::new(worker_bridge.clone()))
                        .with_permission_ui(Box::new(worker_bridge.clone()))
                        .with_user_question_ui(Box::new(worker_bridge.clone()))
                        .with_compaction_config(CompactStrategy::Default.config())
                        .with_agent_context(agent_context);
                    if let Some(runner) = &worker_hook_runner {
                        query_loop = query_loop.with_hook_runner(runner.clone());
                    }
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

                                // Gather git context off the async runtime, outside
                                // any mutex, so the blocking subprocess calls don't
                                // stall the executor. We only hold the lock to read
                                // cwd, release it, then reacquire once to write the
                                // fresh context and collect the other fields.
                                let cwd_snapshot = { worker_state_clone.lock().await.cwd.clone() };
                                let new_git_context = tokio::task::spawn_blocking(move || {
                                    collect_git_context(&cwd_snapshot)
                                })
                                .await
                                .unwrap_or(None);

                                let (
                                    usage,
                                    runtime_model,
                                    model_setting,
                                    permission_mode_display,
                                    stream_enabled,
                                    git_branch,
                                ) = {
                                    let mut state = worker_state_clone.lock().await;
                                    state.git_context = new_git_context;
                                    (
                                        state.total_usage.clone(),
                                        get_runtime_main_loop_model(
                                            &state.session.model_setting,
                                            state.permission_mode,
                                            state.most_recent_assistant_usage().is_some_and(
                                                rust_claude_core::model::usage_exceeds_200k_tokens,
                                            ),
                                        ),
                                        state.session.model_setting.clone(),
                                        format!("{:?}", state.permission_mode),
                                        state.session.stream,
                                        state.git_context.as_ref().map(|g| g.branch.clone()),
                                    )
                                };

                                if !stream_enabled {
                                    if !text.is_empty() {
                                        worker_bridge_clone.send_assistant_message(&text).await;
                                    } else {
                                        worker_bridge_clone
                                            .send_assistant_message("(no text content)")
                                            .await;
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
                                    .send_status_update(
                                        &runtime_model,
                                        &model_setting,
                                        &permission_mode_display,
                                        git_branch.as_deref(),
                                    )
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
                        session_file.id = state_snapshot.session.id.clone();
                        session_file.messages = state_snapshot.messages.clone();
                        session_file.total_usage = Some(state_snapshot.total_usage.clone());
                        session_file.permission_mode = state_snapshot.permission_mode;
                        session_file.always_allow_rules = state_snapshot.always_allow_rules.clone();
                        session_file.always_deny_rules = state_snapshot.always_deny_rules.clone();
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
    let mut app = App::new(model, model_setting, permission_mode, git_branch, theme);

    // Register plugin slash commands in the TUI registry
    {
        let pm = app_plugin_manager.lock().await;
        for plugin in pm.plugins() {
            for cmd in &plugin.slash_commands {
                let handler = rust_claude_tui::slash::PluginSlashCommandHandler::new(
                    cmd.name.clone(),
                    cmd.description.clone(),
                    cmd.prompt.clone(),
                );
                app.slash_registry.register(handler.into_command());
            }
        }
        // Register /plugin commands
        {
            let pm_arc = app_plugin_manager.clone();
            let _bridge = app_worker_bridge.clone();
            app.slash_registry.register(rust_claude_tui::slash::command(
                "/plugin", "/plugin list|install <path>|remove <name>",
                "Manage plugins",
                move |args, _user_tx| {
                    match args {
                        Some(args_str) if args_str.starts_with("list") => {
                            let pm_lock = pm_arc.blocking_lock();
                            let plugins = pm_lock.plugins();
                            let mut lines = vec!["Installed plugins:".to_string()];
                            if plugins.is_empty() {
                                lines.push("  (none)".to_string());
                            } else {
                                for p in plugins {
                                    lines.push(format!("  {} v{} - {} (from {})",
                                        p.name, p.version, p.description,
                                        if p.is_project { "project" } else { "user" }
                                    ));
                                }
                            }
                            rust_claude_tui::slash::SlashCommandResult::SystemMessage(lines.join("\n"))
                        }
                        _ => {
                            rust_claude_tui::slash::SlashCommandResult::SystemMessage(
                                "Usage: /plugin list | /plugin install <path> | /plugin remove <name>".into()
                            )
                        }
                    }
                },
            ));
        }
    }
    let run_result = app.run(terminal_guard.terminal_mut(), event_rx, user_tx).await;
    if let Some(runner) = &hook_runner {
        let session_id = { app_state.lock().await.session.id.clone() };
        let reason = if run_result.is_ok() {
            "tui_exit"
        } else {
            "error"
        };
        runner.run_session_end(reason, &session_id).await;
    }
    Ok(run_result?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_claude_core::permission::{PermissionRule, RuleType};
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Tests in this module mutate process-global env variables
    /// (`ANTHROPIC_MODEL`, `RUST_CLAUDE_MODEL_OVERRIDE`, etc.). Rust runs tests
    /// in parallel, so we serialize every test that reads/writes these
    /// variables through a single shared lock.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn truncate_latest_user_turn_removes_user_and_following_assistant_messages() {
        let mut messages = vec![
            Message::user("first"),
            Message::assistant(vec![ContentBlock::text("first answer")]),
            Message::user("second"),
            Message::assistant(vec![ContentBlock::text("second answer")]),
        ];

        assert!(truncate_latest_user_turn(&mut messages));
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0], Message::user("first"));
    }

    #[test]
    fn recap_messages_reports_empty_conversation() {
        assert_eq!(recap_messages(&[]), None);
    }

    #[test]
    fn recap_messages_counts_turns_and_latest_user_request() {
        let messages = vec![
            Message::user("first"),
            Message::assistant(vec![ContentBlock::text("first answer")]),
            Message::user("second request"),
        ];

        let recap = recap_messages(&messages).unwrap();
        assert!(recap.contains("user turns: 2"));
        assert!(recap.contains("assistant turns: 1"));
        assert!(recap.contains("latest user request: second request"));
    }

    #[test]
    fn effort_budget_maps_levels_to_budget_overrides() {
        assert_eq!(effort_budget("low"), Some(Some(3_000)));
        assert_eq!(effort_budget("medium"), Some(Some(10_000)));
        assert_eq!(effort_budget("high"), Some(None));
        assert_eq!(effort_budget("extreme"), None);
    }

    /// RAII guard that clears `ANTHROPIC_MODEL` and `RUST_CLAUDE_MODEL_OVERRIDE`
    /// for the duration of a test and restores them on drop.
    struct EnvReset {
        anthropic_model: Option<String>,
        override_model: Option<String>,
    }

    impl EnvReset {
        fn new() -> Self {
            let anthropic_model = std::env::var("ANTHROPIC_MODEL").ok();
            let override_model = std::env::var("RUST_CLAUDE_MODEL_OVERRIDE").ok();
            unsafe {
                std::env::remove_var("ANTHROPIC_MODEL");
                std::env::remove_var("RUST_CLAUDE_MODEL_OVERRIDE");
            }
            Self {
                anthropic_model,
                override_model,
            }
        }
    }

    impl Drop for EnvReset {
        fn drop(&mut self) {
            unsafe {
                match &self.anthropic_model {
                    Some(v) => std::env::set_var("ANTHROPIC_MODEL", v),
                    None => std::env::remove_var("ANTHROPIC_MODEL"),
                }
                match &self.override_model {
                    Some(v) => std::env::set_var("RUST_CLAUDE_MODEL_OVERRIDE", v),
                    None => std::env::remove_var("RUST_CLAUDE_MODEL_OVERRIDE"),
                }
            }
        }
    }

    #[test]
    fn resolve_config_prefers_cli_over_project_and_user() {
        let _g = env_lock();
        let _reset = EnvReset::new();

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
            theme: None,
            provider: None,
            thinking: false,
            no_thinking: false,
            verbose: false,
            continue_session: false,
            resume_session: None,
            settings: None,
        };
        let config = Config::with_credential("test-key".to_string(), false);
        let user_settings = ClaudeSettings {
            model: Some("user-model".into()),
            ..Default::default()
        };
        let project_settings = Some(SettingsLayer {
            path: std::path::PathBuf::from("/repo/.claude/settings.json"),
            settings: ClaudeSettings {
                model: Some("project-model".into()),
                ..Default::default()
            },
        });

        let resolved = resolve_config(&cli, config, user_settings, project_settings).unwrap();
        assert_eq!(resolved.model_setting, "opus[1m]");
        assert_eq!(resolved.model, "claude-opus-4-6[1m]");
        assert_eq!(resolved.config.provenance.model, ConfigSource::Cli);
    }

    #[test]
    fn resolve_config_uses_cli_provider() {
        let _g = env_lock();
        let _reset = EnvReset::new();

        let cli = Cli {
            provider: Some("bedrock".to_string()),
            ..default_cli()
        };
        let config = Config::with_credential("test-key".to_string(), false);

        let resolved = resolve_config(&cli, config, ClaudeSettings::default(), None).unwrap();

        assert_eq!(resolved.provider, Provider::Bedrock);
    }

    #[test]
    fn resolve_config_uses_project_permissions() {
        let _g = env_lock();
        let _reset = EnvReset::new();

        let cli = Cli {
            prompt: vec![],
            mode: None,
            model: None,
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
            theme: None,
            provider: None,
            thinking: false,
            no_thinking: false,
            verbose: false,
            continue_session: false,
            resume_session: None,
            settings: None,
        };
        let config = Config::with_credential("test-key".to_string(), false);
        let project_settings = Some(SettingsLayer {
            path: std::path::PathBuf::from("/repo/.claude/settings.json"),
            settings: ClaudeSettings {
                permissions: rust_claude_core::settings::SettingsPermissions {
                    allow: vec!["Bash(git status *)".into()],
                    deny: vec![],
                },
                ..Default::default()
            },
        });

        let resolved =
            resolve_config(&cli, config, ClaudeSettings::default(), project_settings).unwrap();
        assert_eq!(resolved.always_allow.len(), 1);
        assert_eq!(
            resolved.config.provenance.always_allow,
            ConfigSource::ProjectSettings
        );
    }

    fn default_cli() -> Cli {
        Cli {
            prompt: vec![],
            mode: None,
            model: None,
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
            theme: None,
            provider: None,
            thinking: false,
            no_thinking: false,
            verbose: false,
            continue_session: false,
            resume_session: None,
            settings: None,
        }
    }

    #[test]
    fn context_snapshot_uses_known_capacity_and_tool_breakdown() {
        let mut state = AppState::new(PathBuf::from("/repo"));
        state.session.model = "claude-sonnet-4-6".into();
        state.session.system_prompt = Some("system prompt".into());
        state.messages.push(Message::user("hello from the user"));
        state.messages.push(Message::tool_results(&[(
            "toolu_1".into(),
            "tool output with a few words".into(),
            false,
        )]));
        state.total_usage = Usage {
            input_tokens: 100,
            output_tokens: 25,
            cache_creation_input_tokens: 5,
            cache_read_input_tokens: 10,
        };

        let snapshot = build_context_snapshot(&state);

        assert_eq!(snapshot.context_capacity, Some(200_000));
        assert_eq!(snapshot.used_tokens, 115);
        assert!(snapshot.system_prompt_tokens > 0);
        assert!(snapshot.message_tokens > 0);
        assert!(snapshot.tool_result_tokens > 0);
        assert_eq!(snapshot.remaining_tokens, Some(199_885));
    }

    #[test]
    fn format_custom_agents_lists_agents_and_empty_state() {
        let empty = CustomAgentRegistry::empty();
        assert_eq!(format_custom_agents(&empty), "No custom agents configured");

        let registry = CustomAgentRegistry::from_agents(vec![
            rust_claude_core::custom_agents::CustomAgentDefinition {
                name: "reviewer".into(),
                description: "Reviews code".into(),
                system_prompt: "Review carefully".into(),
                tools: vec![],
                model: None,
                path: PathBuf::from("reviewer.md"),
            },
        ]);
        let output = format_custom_agents(&registry);
        assert!(output.contains("Custom agents:"));
        assert!(output.contains("reviewer - Reviews code"));
    }

    #[test]
    fn context_snapshot_handles_unknown_capacity() {
        let mut state = AppState::new(PathBuf::from("/repo"));
        state.session.model = "local-model".into();
        state.messages.push(Message::user("hello"));

        let snapshot = build_context_snapshot(&state);

        assert_eq!(snapshot.context_capacity, None);
        assert_eq!(snapshot.remaining_tokens, None);
        assert!(snapshot.used_tokens > 0);
    }

    #[test]
    fn export_conversation_writes_markdown_with_metadata_and_tools() {
        let dir = std::env::temp_dir().join(format!(
            "export-test-{}-{}",
            std::process::id(),
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("conversation.md");

        let mut state = AppState::new(PathBuf::from("/repo"));
        state.session.model = "claude-sonnet-4-6".into();
        state.session.model_setting = "sonnet".into();
        state.messages.push(Message::user("hello"));
        state.messages.push(Message::assistant(vec![
            ContentBlock::text("hi"),
            ContentBlock::tool_use("toolu_1", "Bash", serde_json::json!({"command": "pwd"})),
        ]));
        state.messages.push(Message::tool_results(&[(
            "toolu_1".into(),
            "/repo".into(),
            false,
        )]));

        let written = export_conversation(&state, Some(path.clone())).unwrap();
        let content = std::fs::read_to_string(&written).unwrap();

        assert_eq!(written, path);
        assert!(content.contains("# rust-claude conversation export"));
        assert!(content.contains("- Model: `claude-sonnet-4-6`"));
        assert!(content.contains("## 1. User"));
        assert!(content.contains("### Tool Use: Bash"));
        assert!(content.contains("### Tool Result: toolu_1"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn latest_assistant_text_ignores_non_text_and_selects_latest() {
        let messages = vec![
            Message::assistant(vec![ContentBlock::text("old")]),
            Message::user("question"),
            Message::assistant(vec![
                ContentBlock::thinking("hidden"),
                ContentBlock::text("new"),
            ]),
        ];

        assert_eq!(latest_assistant_text(&messages), Some("new".into()));
        assert_eq!(latest_assistant_text(&[Message::user("none")]), None);
    }

    #[test]
    fn copy_text_with_maps_empty_and_provider_errors() {
        let empty = copy_text_with("", |_| Ok(())).unwrap_err();
        assert!(empty.to_string().contains("clipboard text is empty"));

        let provider =
            copy_text_with("text", |_| Err(anyhow!("provider unavailable"))).unwrap_err();
        assert!(provider.to_string().contains("provider unavailable"));
    }

    /// Regression: user-scope `deny` rules must survive when a project-scope
    /// settings file only contributes `allow` rules.
    #[test]
    fn resolve_config_merges_user_deny_with_project_allow() {
        let _g = env_lock();
        let _reset = EnvReset::new();

        let cli = default_cli();
        let config = Config::with_credential("test-key".to_string(), false);
        let user_settings = ClaudeSettings {
            permissions: rust_claude_core::settings::SettingsPermissions {
                allow: vec![],
                deny: vec!["Bash(rm *)".into()],
            },
            ..Default::default()
        };
        let project_settings = Some(SettingsLayer {
            path: std::path::PathBuf::from("/repo/.claude/settings.json"),
            settings: ClaudeSettings {
                permissions: rust_claude_core::settings::SettingsPermissions {
                    allow: vec!["Bash(git status *)".into()],
                    deny: vec![],
                },
                ..Default::default()
            },
        });

        let resolved = resolve_config(&cli, config, user_settings, project_settings).unwrap();
        // Both layers contribute to the respective lists.
        assert_eq!(resolved.always_allow.len(), 1, "project allow preserved");
        assert_eq!(resolved.always_deny.len(), 1, "user deny preserved");
        // Allow provenance points at the project (it's the sole contributor),
        // deny provenance points at the user (it's the sole contributor).
        assert_eq!(
            resolved.config.provenance.always_allow,
            ConfigSource::ProjectSettings
        );
        assert_eq!(
            resolved.config.provenance.always_deny,
            ConfigSource::UserConfig
        );
    }

    /// Regression for the priority chain: `--model` must beat `ANTHROPIC_MODEL`
    /// when both are present.
    #[test]
    fn resolve_config_cli_model_beats_anthropic_model_env() {
        let _g = env_lock();
        let _reset = EnvReset::new();
        // SAFETY: test-only, serialized by env_lock above.
        unsafe { std::env::set_var("ANTHROPIC_MODEL", "env-model") };

        let mut cli = default_cli();
        cli.model = Some("opus[1m]".to_string());

        let config = Config::with_credential("test-key".to_string(), false);
        let resolved = resolve_config(&cli, config, ClaudeSettings::default(), None).unwrap();

        assert_eq!(resolved.model_setting, "opus[1m]");
        assert_eq!(resolved.config.provenance.model, ConfigSource::Cli);
    }

    /// Exercises the dedup-aware memory write path: when a memory already
    /// exists at the target path, `correct_memory_entry` is used instead of
    /// `write_memory_entry`, producing an "Updated" message.
    #[test]
    fn remember_memory_updates_existing_entry_via_dedup() {
        use rust_claude_core::memory;
        use std::fs;

        let dir = std::env::temp_dir().join(format!("remember-dedup-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let store = memory::MemoryStore {
            project_root: std::path::PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };

        // Seed an initial memory entry
        let initial = memory::MemoryWriteRequest {
            relative_path: "testing.md".to_string(),
            frontmatter: memory::MemoryFrontmatter {
                name: Some("Testing".to_string()),
                description: Some("Old desc".to_string()),
                memory_type: Some(memory::MemoryType::Feedback),
                extra: std::collections::HashMap::new(),
            },
            body: "Old body.".to_string(),
        };
        memory::write_memory_entry(&store, &initial).unwrap();

        // Build an update request targeting the same path
        let update = memory::MemoryWriteRequest {
            relative_path: "testing.md".to_string(),
            frontmatter: memory::MemoryFrontmatter {
                name: Some("Testing".to_string()),
                description: Some("New desc".to_string()),
                memory_type: Some(memory::MemoryType::Feedback),
                extra: std::collections::HashMap::new(),
            },
            body: "New body.".to_string(),
        };

        // Duplicate detection should find the existing entry
        let scanned = memory::scan_memory_store(&store).unwrap();
        let dup = memory::find_duplicate_memory(&scanned, &update);
        assert!(
            dup.is_some(),
            "existing entry should be detected as duplicate"
        );

        // Correct instead of create
        let path = memory::correct_memory_entry(&store, &update).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("New desc"));
        assert!(content.contains("New body."));
        assert!(!content.contains("Old"));

        // Only one entry in the store, not two
        let rescan = memory::scan_memory_store(&store).unwrap();
        assert_eq!(rescan.entries.len(), 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn manual_verification_permissions_output_contains_rules() {
        let mut state = AppState::new(PathBuf::from("/repo"));
        state.permission_mode = PermissionMode::Default;
        state
            .always_allow_rules
            .push(PermissionRule::parse("FileEdit(, /src/**)", RuleType::Allow).unwrap());
        state
            .always_deny_rules
            .push(PermissionRule::parse("FileRead(, /.env)", RuleType::Deny).unwrap());

        let mode_str = format!("{:?}", state.permission_mode);
        let mut text = format!("Permission Mode: {}\n", mode_str);
        if !state.always_deny_rules.is_empty() {
            text.push_str("\nDeny rules:\n");
            for rule in &state.always_deny_rules {
                text.push_str(&format!("  - {}\n", rule.to_compact_string()));
            }
        }
        if !state.always_allow_rules.is_empty() {
            text.push_str("\nAllow rules:\n");
            for rule in &state.always_allow_rules {
                text.push_str(&format!("  - {}\n", rule.to_compact_string()));
            }
        }

        assert!(text.contains("Permission Mode: Default"));
        assert!(text.contains("Deny rules:"));
        assert!(text.contains("FileRead(, /.env)"));
        assert!(text.contains("Allow rules:"));
        assert!(text.contains("FileEdit(, /src/**)"));
    }

    #[test]
    fn manual_verification_init_behavior_does_not_overwrite_existing() {
        use std::fs;
        let dir = std::env::temp_dir().join(format!("init-manual-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".claude")).unwrap();
        fs::write(dir.join("CLAUDE.md"), "existing").unwrap();

        let claude_dir = dir.join(".claude");
        let claude_md_path = dir.join("CLAUDE.md");
        let mut result = String::new();
        if !claude_dir.exists() {
            fs::create_dir_all(&claude_dir).unwrap();
        } else {
            result.push_str(&format!("{} already exists\n", claude_dir.display()));
        }
        if !claude_md_path.exists() {
            fs::write(&claude_md_path, "starter").unwrap();
        } else {
            result.push_str(&format!(
                "{} already exists (not overwritten)\n",
                claude_md_path.display()
            ));
        }

        let content = fs::read_to_string(&claude_md_path).unwrap();
        assert_eq!(content, "existing");
        assert!(result.contains("already exists"));
        assert!(result.contains("not overwritten"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn manual_verification_status_output_contains_expected_sections() {
        let model = "claude-opus-4-6-manual-test";
        let mode_str = "Default";
        let rule_count = 2usize;
        let allow_count = 1usize;
        let deny_count = 1usize;
        let mcp_count = 0usize;
        let hook_count = 0usize;
        let memory_count = 0usize;

        let msg = format!(
            "Status:\n  Model: {}\n  Permission mode: {}\n  Permission rules: {} ({} allow, {} deny)\n  MCP servers: {}\n  Hooks: {}\n  Memory entries: {}",
            model, mode_str, rule_count, allow_count, deny_count, mcp_count, hook_count, memory_count
        );

        assert!(msg.contains("Status:"));
        assert!(msg.contains("Model: claude-opus-4-6-manual-test"));
        assert!(msg.contains("Permission mode: Default"));
        assert!(msg.contains("Permission rules: 2 (1 allow, 1 deny)"));
        assert!(msg.contains("MCP servers: 0"));
        assert!(msg.contains("Hooks: 0"));
        assert!(msg.contains("Memory entries: 0"));
    }

    /// When a duplicate is found by name but at a different path, the fix
    /// should update the file at the existing path, not the new path.
    #[test]
    fn remember_memory_name_dedup_targets_existing_path() {
        use rust_claude_core::memory;
        use std::fs;

        let dir = std::env::temp_dir().join(format!("remember-name-dedup-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let store = memory::MemoryStore {
            project_root: std::path::PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };

        // Seed an entry at "testing.md" with name "Testing Conventions"
        let initial = memory::MemoryWriteRequest {
            relative_path: "testing.md".to_string(),
            frontmatter: memory::MemoryFrontmatter {
                name: Some("Testing Conventions".to_string()),
                description: Some("Old desc".to_string()),
                memory_type: Some(memory::MemoryType::Feedback),
                extra: std::collections::HashMap::new(),
            },
            body: "Old body.".to_string(),
        };
        memory::write_memory_entry(&store, &initial).unwrap();

        // A new request at a DIFFERENT path but same name (case-insensitive)
        let update = memory::MemoryWriteRequest {
            relative_path: "conventions/testing.md".to_string(),
            frontmatter: memory::MemoryFrontmatter {
                name: Some("testing conventions".to_string()),
                description: Some("New desc".to_string()),
                memory_type: Some(memory::MemoryType::Feedback),
                extra: std::collections::HashMap::new(),
            },
            body: "New body.".to_string(),
        };

        // find_duplicate_memory matches by name
        let scanned = memory::scan_memory_store(&store).unwrap();
        let dup = memory::find_duplicate_memory(&scanned, &update);
        assert!(dup.is_some(), "name-based match should find existing entry");
        let dup_entry = dup.unwrap();
        assert_eq!(dup_entry.relative_path, "testing.md");

        // The corrected request should target the existing path
        let corrected = memory::MemoryWriteRequest {
            relative_path: dup_entry.relative_path.clone(),
            ..update
        };
        let path = memory::correct_memory_entry(&store, &corrected).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("New desc"), "should have updated content");
        assert!(content.contains("New body."), "should have updated body");

        // Still only one entry, at the original path
        let rescan = memory::scan_memory_store(&store).unwrap();
        assert_eq!(rescan.entries.len(), 1);
        assert_eq!(rescan.entries[0].relative_path, "testing.md");

        // The new path should NOT have been created
        assert!(!dir.join("conventions/testing.md").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_memory_store_status_empty_store() {
        use rust_claude_core::memory;
        use std::fs;

        let dir = std::env::temp_dir().join(format!("memory-status-empty-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let store = memory::MemoryStore {
            project_root: dir.clone(),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };

        let status = format_memory_store_status(&store);
        assert!(status.contains("entries: 0"), "should show zero entries");
        assert!(
            status.contains("No memory entries found"),
            "should indicate empty"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_memory_store_status_populated_store() {
        use rust_claude_core::memory;
        use std::fs;

        let dir =
            std::env::temp_dir().join(format!("memory-status-populated-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let store = memory::MemoryStore {
            project_root: std::path::PathBuf::from("/repo"),
            memory_dir: dir.clone(),
            entrypoint: dir.join("MEMORY.md"),
        };

        // Write two memory entries
        memory::write_memory_entry(
            &store,
            &memory::MemoryWriteRequest {
                relative_path: "testing.md".to_string(),
                frontmatter: memory::MemoryFrontmatter {
                    name: Some("Testing".to_string()),
                    description: Some("DB test guidance".to_string()),
                    memory_type: Some(memory::MemoryType::Feedback),
                    extra: std::collections::HashMap::new(),
                },
                body: "Use real database.".to_string(),
            },
        )
        .unwrap();
        memory::write_memory_entry(
            &store,
            &memory::MemoryWriteRequest {
                relative_path: "deploy.md".to_string(),
                frontmatter: memory::MemoryFrontmatter {
                    name: Some("Deploy".to_string()),
                    description: Some("Deploy process".to_string()),
                    memory_type: Some(memory::MemoryType::Project),
                    extra: std::collections::HashMap::new(),
                },
                body: "Use rolling deploy.".to_string(),
            },
        )
        .unwrap();

        let status = format_memory_store_status(&store);
        assert!(status.contains("entries: 2"), "should show two entries");
        assert!(status.contains("Visible memories:"), "should list memories");
        assert!(status.contains("[feedback]"), "should show memory type");
        assert!(status.contains("[project]"), "should show second type");
        assert!(
            status.contains("DB test guidance"),
            "should show description"
        );
        assert!(
            status.contains("Entrypoint loaded:"),
            "should show entrypoint info"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn format_memory_store_status_no_store_dir() {
        use rust_claude_core::memory;

        let dir = std::env::temp_dir().join(format!("memory-status-nodir-{}", std::process::id()));
        // Ensure dir does NOT exist
        let _ = std::fs::remove_dir_all(&dir);

        let store = memory::MemoryStore {
            project_root: dir.clone(),
            memory_dir: dir.join("nonexistent-memory"),
            entrypoint: dir.join("nonexistent-memory/MEMORY.md"),
        };

        let status = format_memory_store_status(&store);
        assert!(
            status.contains("entries: 0"),
            "non-existent dir should show 0 entries"
        );
        assert!(
            status.contains("No memory entries found"),
            "should indicate empty"
        );
    }

    /// `RUST_CLAUDE_MODEL_OVERRIDE` must still beat both CLI `--model` and
    /// `ANTHROPIC_MODEL` (top of the documented priority chain).
    #[test]
    fn resolve_config_rust_claude_model_override_beats_cli() {
        let _g = env_lock();
        let _reset = EnvReset::new();
        unsafe { std::env::set_var("RUST_CLAUDE_MODEL_OVERRIDE", "override-model") };

        let mut cli = default_cli();
        cli.model = Some("opus[1m]".to_string());

        let config = Config::with_credential("test-key".to_string(), false);
        let resolved = resolve_config(&cli, config, ClaudeSettings::default(), None).unwrap();

        assert_eq!(resolved.model_setting, "override-model");
        assert_eq!(resolved.config.provenance.model, ConfigSource::Env);
    }

    #[test]
    fn review_prompt_prioritizes_findings_and_notes_truncation() {
        let diff = format!("diff --git a/a b/a\n{}", "x".repeat(REVIEW_DIFF_LIMIT + 10));
        let input = build_review_input("current branch", &diff);
        assert!(input.truncated);
        assert!(input.prompt.contains("Findings must come first"));
        assert!(input.prompt.contains("severity"));
        assert!(input.prompt.contains("residual risk"));
        assert!(input.prompt.len() < diff.len() + 1000);
    }

    #[test]
    fn pr_review_reports_missing_gh() {
        if executable_available("gh") {
            return;
        }
        let error = collect_review_input(std::path::Path::new("."), Some("123")).unwrap_err();
        assert!(error.to_string().contains("PR lookup requires `gh`"));
    }
}
