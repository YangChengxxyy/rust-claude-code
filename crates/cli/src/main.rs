use std::sync::Arc;

use anyhow::{anyhow, Result};
use clap::Parser;
use rust_claude_api::AnthropicClient;
use rust_claude_core::{
    config::{Config, ConfigError},
    permission::PermissionMode,
    state::AppState,
};
use rust_claude_tools::{BashTool, ToolRegistry};
use tokio::sync::Mutex;

use rust_claude_cli::query_loop::QueryLoop;

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
struct Cli {
    prompt: Vec<String>,

    /// Permission mode: default, accept-edits, bypass, plan, dont-ask
    #[arg(short = 'm', long = "mode")]
    mode: Option<ModeArg>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    println!("rust-claude-code: Rust implementation of Claude Code");

    match Config::load() {
        Ok(config) => {
            let model = std::env::var("RUST_CLAUDE_MODEL_OVERRIDE").unwrap_or_else(|_| config.model.clone());
            let base_url_override = std::env::var("RUST_CLAUDE_BASE_URL").ok();
            let bearer_auth = std::env::var("RUST_CLAUDE_BEARER_AUTH")
                .ok()
                .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
                .unwrap_or(false);

            let mut state = AppState::from_config(cwd.clone(), &config);
            state.session.model = model;
            if let Some(ModeArg(mode)) = cli.mode {
                state.permission_mode = mode;
            }
            println!("Initialized session state from config.");
            println!("cwd: {}", cwd.display());
            println!("model: {}", state.session.model);
            println!("max_tokens: {}", state.session.max_tokens);
            println!("permission_mode: {:?}", state.permission_mode);
            println!("always_allow_rules: {}", state.always_allow_rules.len());
            println!("always_deny_rules: {}", state.always_deny_rules.len());

            if !cli.prompt.is_empty() {
                let app_state = Arc::new(Mutex::new(state));
                let mut client = AnthropicClient::new(config.api_key)?;
                if let Some(base_url) = base_url_override {
                    client = client.with_base_url(base_url);
                }
                if bearer_auth {
                    client = client.with_bearer_auth();
                }
                let mut tools = ToolRegistry::new();
                tools.register(BashTool::new());

                let query_loop = QueryLoop::new(client, tools);
                let prompt = cli.prompt.join(" ");
                let final_message = query_loop.run(app_state, prompt).await?;

                for block in final_message.content {
                    if let rust_claude_core::message::ContentBlock::Text { text } = block {
                        println!("{text}");
                    }
                }
                return Ok(());
            }
        }
        Err(ConfigError::MissingApiKey) => {
            println!("No API key configured yet. Skipping session initialization.");
            println!("cwd: {}", cwd.display());
            if !cli.prompt.is_empty() {
                println!("Cannot run the query loop without ANTHROPIC_API_KEY.");
                return Err(anyhow!("ANTHROPIC_API_KEY is required to run the query loop"));
            }
        }
        Err(error) => return Err(error.into()),
    }

    println!("Pass a prompt argument to run the query loop.");

    Ok(())
}
