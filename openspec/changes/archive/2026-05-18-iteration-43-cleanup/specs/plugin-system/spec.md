## MODIFIED Requirements

### Requirement: Plugin install command
The system SHALL provide a `/plugin install <path>` slash command that installs a plugin from a local directory path. The path SHALL be copied to `~/.claude/plugins/<name>/` after validating `plugin.json`.

#### Scenario: Install plugin from local path
- **WHEN** the user runs `/plugin install /path/to/my-plugin` and the directory contains a valid `plugin.json`
- **THEN** the plugin SHALL be installed to `~/.claude/plugins/my-plugin/` and loaded

#### Scenario: Installed plugin reloads runtime state
- **WHEN** plugin installation succeeds
- **THEN** plugin state SHALL be reloaded so newly declared MCP servers, agents, and slash commands become available without restarting

#### Scenario: Install from invalid path
- **WHEN** the user runs `/plugin install /nonexistent/path`
- **THEN** the system SHALL display an error indicating the path does not exist

#### Scenario: Install plugin with missing plugin.json
- **WHEN** the user runs `/plugin install /path/to/dir` and the directory does not contain `plugin.json`
- **THEN** the system SHALL display an error indicating the manifest is missing

#### Scenario: Install plugin with invalid manifest version
- **WHEN** the user runs `/plugin install /path/to/dir` and `plugin.json` is missing a valid `version`
- **THEN** the system SHALL reject installation with a validation error

#### Scenario: Install target already exists
- **WHEN** the user installs a plugin whose target directory already exists in `~/.claude/plugins/`
- **THEN** the system SHALL fail with a clear conflict error rather than silently overwriting it

### Requirement: Plugin remove command
The system SHALL provide a `/plugin remove <name>` slash command that unloads and removes a user-installed plugin by name. All plugin resources SHALL be cleaned up.

#### Scenario: Remove installed plugin
- **WHEN** the user runs `/plugin remove my-plugin` and `my-plugin` is installed in `~/.claude/plugins/`
- **THEN** the plugin SHALL be unloaded, its directory SHALL be removed from `~/.claude/plugins/`, and plugin state SHALL be reloaded

#### Scenario: Remove nonexistent plugin
- **WHEN** the user runs `/plugin remove nonexistent`
- **THEN** the system SHALL display an error indicating the plugin is not installed

#### Scenario: Project plugin is not deleted
- **WHEN** the active plugin named `team-plugin` comes only from `.claude/plugins/team-plugin/`
- **THEN** `/plugin remove team-plugin` SHALL NOT delete the project plugin directory and SHALL report that only user-installed plugins can be removed
