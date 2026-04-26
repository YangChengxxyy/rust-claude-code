## ADDED Requirements

### Requirement: /permissions command displays active rules
The system SHALL provide a `/permissions` slash command that displays all active permission rules grouped by source (built-in defaults, user config, project settings, session additions). Each rule SHALL show its type (Allow/Deny/Ask), tool name, command pattern, and path pattern if present.

#### Scenario: Display rules from multiple sources
- **WHEN** the user runs `/permissions` and rules exist from user config and project settings
- **THEN** the system SHALL display all rules grouped by source, with each rule showing its type, tool, and patterns

#### Scenario: No custom rules configured
- **WHEN** the user runs `/permissions` and no custom rules are configured beyond the permission mode defaults
- **THEN** the system SHALL display the current permission mode and indicate no custom rules are active

#### Scenario: Display current permission mode
- **WHEN** the user runs `/permissions`
- **THEN** the output SHALL include the current permission mode (e.g., Default, AcceptEdits, Plan) at the top

### Requirement: /init command scaffolds project configuration
The system SHALL provide a `/init` slash command that creates a `.claude/` directory in the project root (git root) with a starter `CLAUDE.md` file. The command SHALL NOT overwrite existing files.

#### Scenario: Initialize in a fresh project
- **WHEN** the user runs `/init` and no `.claude/` directory exists at the project root
- **THEN** the system SHALL create `.claude/` directory and `.claude/CLAUDE.md` with starter content, and display a confirmation message

#### Scenario: .claude directory already exists
- **WHEN** the user runs `/init` and `.claude/` directory already exists at the project root
- **THEN** the system SHALL check for `CLAUDE.md` inside it; if absent, create it; if present, display a message that it already exists without overwriting

#### Scenario: No git root found
- **WHEN** the user runs `/init` and no git repository root can be determined
- **THEN** the system SHALL use the session CWD as the target directory and display a warning that no git root was found

### Requirement: /status command shows consolidated system overview
The system SHALL provide a `/status` slash command that displays a consolidated overview of the current system state including: model name, permission mode, active permission rules count, MCP server count and connection status, hooks count, and memory entries count.

#### Scenario: Full status display
- **WHEN** the user runs `/status` with model `claude-sonnet-4-6`, Default permission mode, 3 custom rules, 2 MCP servers (1 connected, 1 disconnected), 4 hooks, and 12 memory entries
- **THEN** the system SHALL display all of these values in a readable summary format

#### Scenario: Minimal status with defaults
- **WHEN** the user runs `/status` with default configuration, no MCP servers, no hooks, and no memory entries
- **THEN** the system SHALL display the model name, permission mode as Default, and zero counts for MCP, hooks, and memory

#### Scenario: Status includes settings source info
- **WHEN** the user runs `/status` and the model was set from project settings
- **THEN** the output SHALL indicate the source of the model setting (e.g., "Model: claude-sonnet-4-6 (from project settings)")
