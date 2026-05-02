use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::events::UserCommand;
use rust_claude_core::compaction::CompactStrategy;
use rust_claude_core::config::Theme;

/// Metadata for a slash command (used for help output and suggestions).
#[derive(Debug, Clone)]
pub struct SlashCommandMeta {
    pub name: String,
    pub usage: String,
    pub description: String,
}

impl SlashCommandMeta {
    pub fn new(name: &str, usage: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            usage: usage.to_string(),
            description: description.to_string(),
        }
    }
}

/// Result of executing a slash command.
#[derive(Debug, Clone)]
pub enum SlashCommandResult {
    /// Command executed; optionally a system message to display.
    SystemMessage(String),
    /// Command sent a UserCommand to the worker.
    Dispatched,
    /// Command was a fire-and-forget prompt template.
    PromptTemplate(String),
    /// Nothing to do.
    None,
}

/// A dynamically registered slash command.
#[async_trait::async_trait]
pub trait SlashCommand: Send + Sync {
    fn meta(&self) -> SlashCommandMeta;
    async fn execute(
        &self,
        args: Option<&str>,
        user_tx: &mpsc::Sender<UserCommand>,
    ) -> SlashCommandResult;
}

/// A dynamic registry of slash commands.
pub struct SlashCommandRegistry {
    commands: HashMap<String, Box<dyn SlashCommand>>,
}

impl SlashCommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, cmd: Box<dyn SlashCommand>) {
        let meta = cmd.meta();
        self.commands.insert(meta.name.clone(), cmd);
    }

    pub fn unregister(&mut self, name: &str) {
        self.commands.remove(name);
    }

    pub fn contains(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    pub fn all_meta(&self) -> Vec<SlashCommandMeta> {
        let mut metas: Vec<_> = self.commands.values().map(|cmd| cmd.meta()).collect();
        metas.sort_by(|a, b| a.name.cmp(&b.name));
        metas
    }

    pub async fn dispatch(
        &self,
        name: &str,
        args: Option<&str>,
        user_tx: &mpsc::Sender<UserCommand>,
    ) -> Option<SlashCommandResult> {
        let cmd = self.commands.get(name)?;
        Some(cmd.execute(args, user_tx).await)
    }
}

impl Default for SlashCommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Convenience constructors ──

type CmdFn = Arc<
    dyn Fn(Option<&str>, &mpsc::Sender<UserCommand>) -> SlashCommandResult + Send + Sync,
>;

struct FnSlashCommand {
    meta: SlashCommandMeta,
    f: CmdFn,
}

#[async_trait::async_trait]
impl SlashCommand for FnSlashCommand {
    fn meta(&self) -> SlashCommandMeta {
        self.meta.clone()
    }

    async fn execute(
        &self,
        args: Option<&str>,
        user_tx: &mpsc::Sender<UserCommand>,
    ) -> SlashCommandResult {
        (self.f)(args, user_tx)
    }
}

pub fn command<F>(name: &str, usage: &str, description: &str, f: F) -> Box<dyn SlashCommand>
where
    F: Fn(Option<&str>, &mpsc::Sender<UserCommand>) -> SlashCommandResult + Send + Sync + 'static,
{
    Box::new(FnSlashCommand {
        meta: SlashCommandMeta::new(name, usage, description),
        f: Arc::new(f),
    })
}

/// A slash command that sends a UserCommand to the worker.
pub fn user_command_cmd<F>(
    name: &str,
    usage: &str,
    description: &str,
    f: F,
) -> Box<dyn SlashCommand>
where
    F: Fn(Option<&str>) -> UserCommand + Send + Sync + 'static,
{
    command(name, usage, description, move |args: Option<&str>, user_tx| {
        let cmd = f(args);
        let result = user_tx.try_send(cmd);
        if result.is_ok() {
            SlashCommandResult::Dispatched
        } else {
            SlashCommandResult::SystemMessage("Failed to dispatch command".into())
        }
    })
}

/// A slash command that sends a prompt template to the agent.
pub fn prompt_cmd(name: &str, description: &str, prompt_template: &str) -> Box<dyn SlashCommand> {
    let prompt = prompt_template.to_string();
    command(
        name,
        &format!("{name} [args]"),
        description,
        move |args: Option<&str>, user_tx| {
            let resolved = if let Some(a) = args {
                prompt.replace("{args}", a)
            } else {
                prompt.clone()
            };
            let _ = user_tx.try_send(UserCommand::Prompt(resolved));
            SlashCommandResult::Dispatched
        },
    )
}

/// Register all 32 built-in slash commands.
pub fn register_builtin_commands(registry: &mut SlashCommandRegistry) {
    // ── Session & lifecycle ──
    registry.register(user_command_cmd(
        "/clear", "/clear [keep-context]", "Clear the chat transcript",
        |args| UserCommand::Prompt(format!("/clear {}", args.unwrap_or(""))),
    ));
    registry.register(user_command_cmd(
        "/compact", "/compact [strategy]", "Compact conversation history",
        |args| {
            let strategy = match args {
                Some("aggressive") => CompactStrategy::Aggressive,
                Some("preserve-recent") => CompactStrategy::PreserveRecent,
                _ => CompactStrategy::Default,
            };
            UserCommand::Compact(strategy)
        },
    ));
    registry.register(user_command_cmd(
        "/resume", "/resume [session-id]", "Resume a previous session",
        |args| UserCommand::ListSessions, // simplified — actual resume needs interaction
    ));
    registry.register(command(
        "/exit", "/exit", "Exit the application",
        |_, _| SlashCommandResult::SystemMessage("Use Ctrl+C to exit".into()),
    ));

    // ── Mode & model ──
    registry.register(user_command_cmd(
        "/mode", "/mode [default|accept-edits|bypass|plan|dont-ask]", "Set permission mode",
        |args| UserCommand::SetMode(args.unwrap_or("").to_string()),
    ));
    registry.register(user_command_cmd(
        "/model", "/model [name]", "Set model",
        |args| UserCommand::SetModel(args.unwrap_or("").to_string()),
    ));
    registry.register(user_command_cmd(
        "/plan", "/plan [description]", "Enter plan mode",
        |args| UserCommand::EnterPlan { description: args.map(|s| s.to_string()) },
    ));

    // ── Info & diagnostics ──
    registry.register(user_command_cmd("/config", "/config", "Show effective configuration", |_| UserCommand::ShowConfig));
    registry.register(user_command_cmd("/cost", "/cost", "Show cumulative usage cost", |_| UserCommand::ShowCost));
    registry.register(user_command_cmd("/diff", "/diff", "Show workspace git diff", |_| UserCommand::ShowDiff));
    registry.register(user_command_cmd("/doctor", "/doctor", "Diagnose environment", |_| UserCommand::ShowDoctor));
    registry.register(user_command_cmd("/status", "/status", "Show status overview", |_| UserCommand::ShowStatus));
    registry.register(user_command_cmd("/context", "/context", "Show context window usage", |_| UserCommand::ShowContext));
    registry.register(user_command_cmd("/keybindings", "/keybindings", "Show keyboard shortcuts", |_| UserCommand::ShowKeybindings));

    // ── Memory ──
    registry.register(user_command_cmd("/memory", "/memory", "Show memory", |_| UserCommand::ShowMemory));

    // ── Hooks ──
    registry.register(user_command_cmd("/hooks", "/hooks", "Show configured hooks", |_| UserCommand::ShowHooks));

    // ── MCP ──
    registry.register(user_command_cmd("/mcp", "/mcp", "Show MCP server status", |_| UserCommand::ShowMcp));

    // ── Permissions ──
    registry.register(user_command_cmd("/permissions", "/permissions", "Manage permissions", |_| UserCommand::ShowPermissions));

    // ── Session management ──
    registry.register(user_command_cmd("/init", "/init", "Initialize project CLAUDE.md", |_| UserCommand::InitProject));
    registry.register(user_command_cmd("/rename", "/rename [name]", "Rename current session", |args| {
        UserCommand::RenameSession { name: args.unwrap_or("").to_string() }
    }));
    registry.register(user_command_cmd("/rewind", "/rewind", "Undo last user turn", |_| UserCommand::Rewind));
    registry.register(user_command_cmd("/recap", "/recap", "Summarize current session", |_| UserCommand::Recap));
    registry.register(user_command_cmd("/add-dir", "/add-dir <path>", "Add workspace directory", |args| {
        UserCommand::AddDirectory { path: std::path::PathBuf::from(args.unwrap_or("")) }
    }));
    registry.register(user_command_cmd("/branch", "/branch [name]", "Create conversation branch", |args| {
        UserCommand::BranchConversation { name: args.map(|s| s.to_string()) }
    }));

    // ── Review ──
    registry.register(user_command_cmd("/review", "/review [target]", "Review code changes", |args| {
        UserCommand::Review { target: args.map(|s| s.to_string()) }
    }));

    // ── Agents ──
    registry.register(user_command_cmd("/agents", "/agents", "List custom agents", |_| UserCommand::ShowAgents));

    // ── Auth ──
    registry.register(user_command_cmd("/login", "/login", "Check authentication status", |_| UserCommand::LoginStatus));
    registry.register(user_command_cmd("/logout", "/logout", "Clear local credentials", |_| UserCommand::Logout));

    // ── Preferences ──
    registry.register(user_command_cmd("/effort", "/effort [low|medium|high]", "Set model effort", |args| {
        UserCommand::SetEffort { level: args.unwrap_or("").to_string() }
    }));
    registry.register(user_command_cmd("/theme", "/theme [name|reload]", "Switch or reload theme", |args| {
        match args {
            Some("reload") | Some("load-custom") => UserCommand::LoadCustomTheme,
            Some(name) => {
                let theme = match name.to_lowercase().as_str() {
                    "dark" | "builtin-dark" => Theme::Dark,
                    "light" | "builtin-light" => Theme::Light,
                    _ => Theme::Dark,
                };
                UserCommand::SetTheme(theme)
            }
            None => UserCommand::SetTheme(Theme::Dark),
        }
    }));

    // ── Misc ──
    registry.register(user_command_cmd("/export", "/export [path]", "Export conversation", |args| {
        UserCommand::ExportConversation { path: args.map(std::path::PathBuf::from) }
    }));
    registry.register(user_command_cmd("/copy", "/copy", "Copy last assistant reply", |_| UserCommand::CopyLatestAssistant));

    // ── Help ──
    registry.register(crate::slash::command(
        "/help", "/help", "Show help for available commands",
        |_, _| SlashCommandResult::SystemMessage("Type /help — list is built from registry".into()),
    ));

    // ── Todo ──
    registry.register(crate::slash::command(
        "/todo", "/todo", "Show or manage task list",
        |_, _| SlashCommandResult::SystemMessage("Toggle task panel with Ctrl+T".into()),
    ));

    // ── Plugin commands (registered elsewhere; here as stubs) ──
    // /plugin list, /plugin install, /plugin remove, /reload-plugins are registered
    // dynamically by the plugin system via register_plugin_commands()
}

/// A slash command from a plugin manifest.
pub struct PluginSlashCommandHandler {
    name: String,
    description: String,
    prompt_template: String,
}

impl PluginSlashCommandHandler {
    pub fn new(name: String, description: String, prompt_template: String) -> Self {
        Self { name, description, prompt_template }
    }

    pub fn into_command(self) -> Box<dyn SlashCommand> {
        let prompt = self.prompt_template;
        command(
            &self.name,
            &format!("{} [args]", self.name),
            &self.description,
            move |args: Option<&str>, user_tx| {
                let resolved = if let Some(a) = args {
                    prompt.replace("{args}", a)
                } else {
                    prompt.clone()
                };
                let _ = user_tx.try_send(UserCommand::Prompt(resolved));
                SlashCommandResult::Dispatched
            },
        )
    }
}
