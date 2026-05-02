## Purpose

Define the plugin system including manifest format, discovery, lifecycle management, and plugin management commands.

## Requirements

### Requirement: Plugin manifest format
The system SHALL support a `plugin.json` manifest file that declares a plugin's name, version, description, MCP server configurations, custom agent definitions, and slash command definitions.

#### Scenario: Valid plugin manifest
- **WHEN** a `plugin.json` file contains `name`, `version`, `description`, and at least one of `mcp_servers`, `custom_agents`, or `slash_commands`
- **THEN** the plugin SHALL be successfully loaded

#### Scenario: Missing required fields
- **WHEN** a `plugin.json` file is missing `name` or `version`
- **THEN** the plugin loader SHALL reject that manifest with a validation error

#### Scenario: Manifest declares MCP server
- **WHEN** a `plugin.json` declares an MCP server with `transport_type: "stdio"`, `command`, and `args`
- **THEN** the plugin loader SHALL add that server configuration to the MCP manager at load time

#### Scenario: Manifest declares custom agents
- **WHEN** a `plugin.json` declares custom agents in the `custom_agents` array
- **THEN** the plugin loader SHALL register those agents in the custom agent registry

#### Scenario: Manifest declares slash commands
- **WHEN** a `plugin.json` declares slash commands with `name` and `prompt`
- **THEN** the plugin loader SHALL register those commands in the slash command registry

### Requirement: Plugin discovery from user and project directories
The system SHALL discover plugins by scanning `~/.claude/plugins/` and `.claude/plugins/` (relative to the project root). Each subdirectory containing a `plugin.json` file SHALL be treated as a plugin.

#### Scenario: Discover plugin in user directory
- **WHEN** `~/.claude/plugins/my-plugin/plugin.json` exists
- **THEN** the plugin loader SHALL discover and attempt to load `my-plugin`

#### Scenario: Discover plugin in project directory
- **WHEN** `.claude/plugins/team-plugin/plugin.json` exists
- **THEN** the plugin loader SHALL discover and attempt to load `team-plugin`

#### Scenario: Empty plugin directories
- **WHEN** neither `~/.claude/plugins/` nor `.claude/plugins/` contain any `plugin.json` files
- **THEN** the plugin loader SHALL return an empty registry without error

#### Scenario: Project plugin overrides user plugin
- **WHEN** both `~/.claude/plugins/shared/` and `.claude/plugins/shared/` contain a `plugin.json`
- **THEN** the project-level plugin SHALL take precedence

### Requirement: Plugin lifecycle management
The system SHALL support loading plugins at startup, reloading all plugins at runtime via `/reload-plugins`, and cleaning up plugin resources on shutdown. Loading a plugin SHALL start its declared MCP servers, register its custom agents, and register its slash commands.

#### Scenario: Plugin loaded at startup
- **WHEN** the application starts with a valid plugin in `~/.claude/plugins/`
- **THEN** the plugin's MCP servers SHALL be started, custom agents SHALL appear in `/agents`, and slash commands SHALL appear in `/help`

#### Scenario: Plugin reload
- **WHEN** the user runs `/reload-plugins`
- **THEN** all existing plugins SHALL be unloaded (MCP servers stopped, agents and commands unregistered), then all plugins SHALL be re-discovered and reloaded

#### Scenario: Plugin cleanup on unload
- **WHEN** a plugin is unloaded (via remove or reload)
- **THEN** its MCP server connections SHALL be terminated, its custom agents SHALL be removed from the registry, and its slash commands SHALL be unregistered

### Requirement: Plugin install command
The system SHALL provide a `/plugin install <path>` slash command that installs a plugin from a local directory path. The path SHALL be copied or symlinked to `~/.claude/plugins/<name>/`.

#### Scenario: Install plugin from local path
- **WHEN** the user runs `/plugin install /path/to/my-plugin` and the directory contains a valid `plugin.json`
- **THEN** the plugin SHALL be installed to `~/.claude/plugins/my-plugin/` and loaded

#### Scenario: Install from invalid path
- **WHEN** the user runs `/plugin install /nonexistent/path`
- **THEN** the system SHALL display an error indicating the path does not exist

#### Scenario: Install plugin with missing plugin.json
- **WHEN** the user runs `/plugin install /path/to/dir` and the directory does not contain `plugin.json`
- **THEN** the system SHALL display an error indicating the manifest is missing

### Requirement: Plugin list command
The system SHALL provide a `/plugin list` slash command that displays all currently loaded plugins with their name, version, description, and source path.

#### Scenario: List loaded plugins
- **WHEN** the user runs `/plugin list` and two plugins are loaded
- **THEN** the output SHALL list both plugins with name, version, and description

#### Scenario: List empty plugins
- **WHEN** the user runs `/plugin list` and no plugins are loaded
- **THEN** the output SHALL display "No plugins installed"

### Requirement: Plugin remove command
The system SHALL provide a `/plugin remove <name>` slash command that unloads and removes an installed plugin by name. All plugin resources (MCP servers, agents, slash commands) SHALL be cleaned up.

#### Scenario: Remove installed plugin
- **WHEN** the user runs `/plugin remove my-plugin` and `my-plugin` is installed
- **THEN** the plugin SHALL be unloaded and its directory SHALL be removed from `~/.claude/plugins/`

#### Scenario: Remove nonexistent plugin
- **WHEN** the user runs `/plugin remove nonexistent`
- **THEN** the system SHALL display an error indicating the plugin is not installed

### Requirement: Plugin slash commands resolve to agent prompts
A slash command declared by a plugin SHALL, when invoked by the user, submit the command's `prompt` field as a user message to the agent.

#### Scenario: Plugin slash command submits prompt
- **WHEN** the user types `/deploy` and a plugin declares `{ "name": "/deploy", "prompt": "Deploy this project to production" }`
- **THEN** the agent SHALL receive "Deploy this project to production" as a user message

#### Scenario: Plugin slash command with arguments
- **WHEN** the user types `/deploy staging` and the plugin prompt template is "Deploy this project to {0}"
- **THEN** the agent SHALL receive "Deploy this project to staging"
