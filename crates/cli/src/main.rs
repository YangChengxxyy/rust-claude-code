use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Parser;
use rust_claude_api::AnthropicClient;
use rust_claude_core::{
    claude_md,
    config::{Config, ConfigError, ConfigOverrides, ConfigSource},
    git::collect_git_context,
    hooks::HooksConfig,
    mcp_config::McpServersConfig,
    message::ContentBlock,
    model::get_runtime_main_loop_model,
    permission::PermissionMode,
    settings::{ClaudeSettings, ParsedPermissions, SettingsLayer},
    state::AppState,
};
use rust_claude_mcp::{McpManager, McpManagerConfig};
use rust_claude_tools::{
    AgentContext, AgentTool, BashTool, FileEditTool, FileReadTool, FileWriteTool, GlobTool,
    GrepTool, TaskTool, ToolRegistry, register_mcp_tools,
};
use rust_claude_tui::{App, TerminalGuard, TuiBridge, UserCommand};
use tokio::sync::{mpsc, Mutex};

use rust_claude_cli::compaction::CompactionService;
use rust_claude_cli::hooks::HookRunner;
use rust_claude_cli::query_loop::QueryLoop;
use rust_claude_cli::session::{self, SessionFile};
use rust_claude_cli::system_prompt;
use rust_claude_core::compaction::CompactionConfig;

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

    #[arg(long = "thinking", conflicts_with = "no_thinking")]
    thinking: bool,

    #[arg(long = "no-thinking")]
    no_thinking: bool,

    #[arg(long = "verbose")]
    verbose: bool,

    #[arg(long = "continue", short = 'c')]
    continue_session: bool,

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
        return Err(anyhow!("--system-prompt and --system-prompt-file are mutually exclusive"));
    }

    let merged_settings = merge_settings_layers(user_settings.clone(), project_settings.as_ref());
    let project_permissions = project_settings
        .as_ref()
        .map(|layer| permissions_from_settings(&layer.settings))
        .transpose()?;
    let user_permissions = permissions_from_settings(&user_settings)?;

    let mut overrides = ConfigOverrides::default();

    if let Some(model) = merged_settings.model.clone() {
        let source = if project_settings.as_ref().and_then(|layer| layer.settings.model.as_ref()).is_some() {
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
            Some(std::fs::read_to_string(path).map_err(|e| anyhow!("failed to read --system-prompt-file '{}': {e}", path))?),
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
        let append = std::fs::read_to_string(append_path)
            .map_err(|e| anyhow!("failed to read --append-system-prompt-file '{}': {e}", append_path))?;
        let base = overrides.system_prompt.value.clone().flatten().or_else(|| config.system_prompt.clone()).unwrap_or_default();
        overrides.system_prompt.set(Some(if base.is_empty() { append } else { format!("{base}\n\n{append}") }), ConfigSource::Cli);
    } else if let Some(append) = &cli.append_system_prompt {
        let base = overrides.system_prompt.value.clone().flatten().or_else(|| config.system_prompt.clone()).unwrap_or_default();
        overrides.system_prompt.set(Some(if base.is_empty() { append.clone() } else { format!("{base}\n\n{append}") }), ConfigSource::Cli);
    }

    config = config.apply_overrides(overrides);

    let model_setting = config.model.clone();
    let permission_mode = config.permission_mode;
    let model = get_runtime_main_loop_model(&model_setting, permission_mode, false);

    let print_mode = cli.print || !cli.prompt.is_empty();
    let output_json = cli.output_format.as_deref() == Some("json");
    let allowed_tools = cli.allowed_tools.clone().unwrap_or_default();
    let disallowed_tools = cli.disallowed_tools.clone().unwrap_or_default();

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
        config,
        project_settings,
    })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    println!("rust-claude-code: Rust implementation of Claude Code");

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
            let merged_settings = merge_settings_layers(user_settings.clone(), project_settings.as_ref());
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
        println!("permissions_source: {} / {}", resolved.config.provenance.always_allow, resolved.config.provenance.always_deny);
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

    // Build hook runner from merged settings
    let hook_runner = if resolved.hooks_config.is_empty() {
        None
    } else {
        Some(Arc::new(HookRunner::new(resolved.hooks_config.clone(), cwd.clone())))
    };

    // Initialize MCP servers (if any configured)
    let mcp_manager = if resolved.mcp_servers.is_empty() {
        Arc::new(McpManager::empty())
    } else {
        if resolved.verbose {
            println!("MCP: starting {} configured server(s)...", resolved.mcp_servers.len());
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
                        println!("  {} (connected, {} tools)", status.name, status.tools.len());
                    }
                    rust_claude_core::mcp_config::McpServerState::Failed(msg) => {
                        println!("  {} (failed: {})", status.name, msg);
                    }
                    rust_claude_core::mcp_config::McpServerState::Pending => {
                        println!("  {} (pending)", status.name);
                    }
                }
            }
        }
        Arc::new(manager)
    };

    if resolved.system_prompt.is_none() {
        let mut tools_for_prompt = build_tools();
        register_mcp_tools(&mut tools_for_prompt, &mcp_manager);
        tools_for_prompt.apply_tool_filters(&resolved.allowed_tools, &resolved.disallowed_tools);
        let git_context = { app_state.lock().await.git_context.clone() };
        let composed = system_prompt::build_system_prompt(
            &cwd,
            &tools_for_prompt,
            &claude_md_files,
            git_context.as_ref(),
            None,
        );
        let mut state = app_state.lock().await;
        state.session.system_prompt = Some(composed);
    }

    if resolved.print_mode {
        let prompt = cli.prompt.join(" ");
        let client = build_client(&resolved.api_key, resolved.base_url.clone(), resolved.bearer_auth)?;
        let mut tools = build_tools();
        register_mcp_tools(&mut tools, &mcp_manager);
        tools.apply_tool_filters(&resolved.allowed_tools, &resolved.disallowed_tools);
        let agent_context = build_agent_context(Arc::new(client.clone()));
        let mut query_loop = QueryLoop::new(client, tools)
            .with_compaction_config(CompactionConfig::default())
            .with_agent_context(agent_context);
        if let Some(max_turns) = resolved.max_turns {
            query_loop = query_loop.with_max_rounds(max_turns);
        }
        if let Some(runner) = &hook_runner {
            query_loop = query_loop.with_hook_runner(runner.clone());
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
        run_tui(app_state, resolved.config.clone(), allowed_tools, disallowed_tools, hook_runner, mcp_manager).await
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

fn build_agent_context(
    client: Arc<dyn rust_claude_api::ModelClient>,
) -> AgentContext {
    AgentContext {
        tool_registry_factory: Arc::new(build_tools),
        run_subagent: Arc::new(move |prompt, allowed_tools, app_state, current_depth, max_depth| {
            let client = client.clone();
            Box::pin(async move {
                let mut tools = build_tools();
                if !allowed_tools.is_empty() {
                    tools.apply_tool_filters(&allowed_tools, &[]);
                }
                let query_loop = QueryLoop::new(client.clone(), tools).with_max_rounds(5).with_agent_context(AgentContext {
                    tool_registry_factory: Arc::new(build_tools),
                    run_subagent: Arc::new(|_, _, _, _, _| {
                        Box::pin(async {
                            Err(rust_claude_tools::ToolError::Execution(
                                "nested agent runner unavailable".to_string(),
                            ))
                        })
                    }),
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
        }),
        current_depth: 0,
        max_depth: 3,
    }
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
            rust_claude_core::mcp_config::McpServerState::Failed(msg) => format!("failed: {}", msg),
            rust_claude_core::mcp_config::McpServerState::Pending => "pending".to_string(),
        };

        text.push_str(&format!("\n  {} ({})\n", status.name, state_str));

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

fn build_tools() -> ToolRegistry {
    let mut tools = ToolRegistry::new();
    tools.register(AgentTool::new());
    tools.register(BashTool::new());
    tools.register(FileReadTool::new());
    tools.register(FileEditTool::new());
    tools.register(FileWriteTool::new());
    tools.register(GlobTool::new());
    tools.register(GrepTool::new());
    tools.register(TaskTool::new());
    tools
}

async fn run_tui(
    app_state: Arc<Mutex<AppState>>,
    config: Config,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
    hook_runner: Option<Arc<HookRunner>>,
    mcp_manager: Arc<McpManager>,
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

    let (event_tx, event_rx) = mpsc::channel(128);
    let (user_tx, mut user_rx) = mpsc::channel::<UserCommand>(16);

    let bridge = TuiBridge::new(event_tx);
    let worker_bridge = bridge.clone();
    let worker_state = app_state.clone();

    worker_bridge
        .send_status_update(&model, &model_setting, &permission_mode, git_branch.as_deref())
        .await;

    let worker_hook_runner = hook_runner;
    let worker_mcp_manager = mcp_manager.clone();
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
                        Ok(result) => worker_bridge.send_compaction_complete(result).await,
                        Err(e) => worker_bridge.send_error(&format!("Compaction failed: {e}")).await,
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

                    let (runtime_model, model_setting, permission_mode_display, git_branch) = {
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
                                    let diff = String::from_utf8_lossy(&output.stdout).trim().to_string();
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
                UserCommand::Prompt(prompt) => {
                    let client = match build_client(&config.api_key, base_url.clone(), bearer_auth) {
                        Ok(client) => client,
                        Err(error) => {
                            worker_bridge.send_error(&error.to_string()).await;
                            continue;
                        }
                    };

                    let mut tools = build_tools();
                    register_mcp_tools(&mut tools, &worker_mcp_manager);
                    tools.apply_tool_filters(&allowed_tools, &disallowed_tools);
                    let agent_context = build_agent_context(Arc::new(client.clone()));
                    let mut query_loop = QueryLoop::new(client, tools)
                        .with_bridge(worker_bridge.clone())
                        .with_compaction_config(compaction_config.clone())
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
                                let cwd_snapshot = {
                                    worker_state_clone.lock().await.cwd.clone()
                                };
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
                                            state
                                                .most_recent_assistant_usage()
                                                .is_some_and(rust_claude_core::model::usage_exceeds_200k_tokens),
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
    let mut app = App::new(model, model_setting, permission_mode, git_branch);
    app.run(terminal_guard.terminal_mut(), event_rx, user_tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// Tests in this module mutate process-global env variables
    /// (`ANTHROPIC_MODEL`, `RUST_CLAUDE_MODEL_OVERRIDE`, etc.). Rust runs tests
    /// in parallel, so we serialize every test that reads/writes these
    /// variables through a single shared lock.
    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
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
            Self { anthropic_model, override_model }
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
            thinking: false,
            no_thinking: false,
            verbose: false,
            continue_session: false,
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
            thinking: false,
            no_thinking: false,
            verbose: false,
            continue_session: false,
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

        let resolved = resolve_config(&cli, config, ClaudeSettings::default(), project_settings).unwrap();
        assert_eq!(resolved.always_allow.len(), 1);
        assert_eq!(resolved.config.provenance.always_allow, ConfigSource::ProjectSettings);
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
            thinking: false,
            no_thinking: false,
            verbose: false,
            continue_session: false,
            settings: None,
        }
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
        assert_eq!(resolved.config.provenance.always_allow, ConfigSource::ProjectSettings);
        assert_eq!(resolved.config.provenance.always_deny, ConfigSource::UserConfig);
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
        let resolved =
            resolve_config(&cli, config, ClaudeSettings::default(), None).unwrap();

        assert_eq!(resolved.model_setting, "opus[1m]");
        assert_eq!(resolved.config.provenance.model, ConfigSource::Cli);
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
        let resolved =
            resolve_config(&cli, config, ClaudeSettings::default(), None).unwrap();

        assert_eq!(resolved.model_setting, "override-model");
        assert_eq!(resolved.config.provenance.model, ConfigSource::Env);
    }
}
