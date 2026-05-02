## MODIFIED Requirements

### Requirement: Slash commands are registered through a unified command registry
The TUI SHALL register slash commands through a unified dynamic command registry backed by a `SlashCommand` trait so that help output, command dispatch metadata, command validation, and slash suggestions are derived from the same self-describing command definitions. Commands SHALL be registered at runtime via `SlashCommandRegistry::register()` rather than via a compile-time `const` array.

#### Scenario: /help lists newly added commands
- **WHEN** the user runs `/help`
- **THEN** the help output SHALL include all registered commands including built-in commands and dynamically registered plugin commands

#### Scenario: Unknown slash command is rejected consistently
- **WHEN** the user enters an unrecognized slash command
- **THEN** the command dispatcher SHALL return a consistent unknown-command error message

#### Scenario: Slash suggestions reuse registered command definitions
- **WHEN** the input buffer begins with `/`
- **THEN** the suggestion overlay SHALL source command candidates from the same dynamic registry used by `/help` and command dispatch, including both built-in and plugin-provided commands

#### Scenario: Command registry supports dynamic registration
- **WHEN** a slash command implementing the `SlashCommand` trait is registered at runtime or initialization time
- **THEN** command validation, help output, and slash suggestions SHALL reflect that command without requiring a separate static command list update

#### Scenario: Plugin commands are registered dynamically
- **WHEN** a plugin manifest declares slash commands and the plugin is loaded
- **THEN** those commands SHALL be registered in the same registry as built-in commands and SHALL appear in suggestions and help output

#### Scenario: Unloading plugin removes its commands
- **WHEN** a plugin is unloaded
- **THEN** its registered slash commands SHALL be removed from the registry and SHALL no longer appear in suggestions or help output
