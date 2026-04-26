use std::io::{self, Stdout};
use std::sync::Once;

use crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub mod app;
pub mod bridge;
pub mod diff;
pub mod events;
pub mod highlight;
pub mod streaming_markdown;
pub mod theme;
pub mod ui;

pub use app::{App, PermissionDialog};
pub use bridge::TuiBridge;
pub use events::{AppEvent, ChatMessage, PermissionResponse, UserCommand};

static PANIC_HOOK_ONCE: Once = Once::new();

/// RAII terminal guard that restores the terminal on drop.
pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    /// Initialize the terminal for full-screen TUI usage.
    pub fn new() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste
        )?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        install_panic_hook();
        Ok(Self { terminal })
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableBracketedPaste,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}

fn install_panic_hook() {
    PANIC_HOOK_ONCE.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = disable_raw_mode();
            let mut stdout = io::stdout();
            let _ = execute!(
                stdout,
                LeaveAlternateScreen,
                DisableBracketedPaste,
                DisableMouseCapture
            );
            default_hook(panic_info);
        }));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_guard_type_exists() {
        let _ = std::mem::size_of::<TerminalGuard>();
    }
}
