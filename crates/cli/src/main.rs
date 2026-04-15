use anyhow::Result;
use rust_claude_core::{
    config::{Config, ConfigError},
    state::AppState,
};

fn main() -> Result<()> {
    let cwd = std::env::current_dir()?;

    println!("rust-claude-code: Rust implementation of Claude Code");

    match Config::load() {
        Ok(config) => {
            let state = AppState::from_config(cwd.clone(), &config);
            println!("Initialized session state from config.");
            println!("cwd: {}", cwd.display());
            println!("model: {}", state.session.model);
            println!("max_tokens: {}", state.session.max_tokens);
            println!("permission_mode: {:?}", state.permission_mode);
            println!("always_allow_rules: {}", state.always_allow_rules.len());
            println!("always_deny_rules: {}", state.always_deny_rules.len());
        }
        Err(ConfigError::MissingApiKey) => {
            println!("No API key configured yet. Skipping session initialization.");
            println!("cwd: {}", cwd.display());
        }
        Err(error) => return Err(error.into()),
    }

    println!("Query loop is not implemented yet. See doc/requirement.md for the plan.");

    Ok(())
}
