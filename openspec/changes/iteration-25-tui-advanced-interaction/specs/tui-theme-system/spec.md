## ADDED Requirements

### Requirement: Built-in themes can be switched at runtime
The TUI SHALL support switching between built-in `dark` and `light` themes at runtime.

#### Scenario: Switch to light theme
- **WHEN** the user runs `/theme light`
- **THEN** the active palette SHALL change to the light theme immediately
- **AND** the TUI SHALL repaint using the new palette without requiring restart

#### Scenario: Switch to dark theme
- **WHEN** the user runs `/theme dark`
- **THEN** the active palette SHALL change to the dark theme immediately
- **AND** the TUI SHALL persist the selected built-in theme using existing configuration persistence

### Requirement: Custom theme file can be loaded
The TUI SHALL support loading a custom theme palette from `~/.config/rust-claude-code/theme.json`.

#### Scenario: Load valid custom theme
- **WHEN** the user runs `/theme custom` and the custom theme file exists with valid required colors
- **THEN** the TUI SHALL apply the custom palette immediately
- **AND** the TUI SHALL display a success message

#### Scenario: Custom theme file is missing
- **WHEN** the user runs `/theme custom` and the custom theme file does not exist
- **THEN** the TUI SHALL display a clear error and keep the current theme unchanged

#### Scenario: Custom theme file is invalid
- **WHEN** the user runs `/theme custom` and the custom theme file contains invalid JSON or invalid color values
- **THEN** the TUI SHALL display a clear parse or validation error and keep the current theme unchanged

### Requirement: Available themes can be listed
The TUI SHALL support listing available theme choices.

#### Scenario: List themes
- **WHEN** the user runs `/theme` without arguments
- **THEN** the TUI SHALL show the active theme and the available choices `dark`, `light`, and `custom`
