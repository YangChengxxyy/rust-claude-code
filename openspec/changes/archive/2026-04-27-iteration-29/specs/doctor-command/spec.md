## ADDED Requirements

### Requirement: The TUI SHALL provide a `/doctor` command
The TUI slash command surface SHALL include `/doctor` as a first-party command that produces an environment diagnostic report.

#### Scenario: Help lists doctor command
- **WHEN** the user runs `/help` or views slash command suggestions
- **THEN** `/doctor` appears with a concise description of the diagnostic report

#### Scenario: Doctor command is dispatched
- **WHEN** the user enters `/doctor`
- **THEN** the TUI dispatches a diagnostic command instead of sending `/doctor` as a normal prompt

### Requirement: `/doctor` SHALL check API configuration and connectivity readiness
The diagnostic report SHALL include the effective model, credential availability, base URL source, auth mode, and whether an API client can be prepared for use.

#### Scenario: API configuration is valid
- **WHEN** required API configuration is present and client setup succeeds
- **THEN** `/doctor` marks API readiness as passing or available

#### Scenario: API credentials are missing
- **WHEN** no supported credential source is available
- **THEN** `/doctor` reports API readiness as failing with a remediation hint

### Requirement: `/doctor` SHALL validate configuration files
The diagnostic report SHALL validate project and user configuration files that affect model, permission, settings, hooks, MCP, and memory behavior.

#### Scenario: Configuration file parses successfully
- **WHEN** a relevant configuration file exists and parses successfully
- **THEN** `/doctor` reports the file path and a passing status

#### Scenario: Configuration file is malformed
- **WHEN** a relevant configuration file exists but cannot be parsed
- **THEN** `/doctor` reports the file path, failing status, and parse error summary

### Requirement: `/doctor` SHALL report MCP server configuration status
The diagnostic report SHALL include configured MCP servers, whether their configuration can be parsed, and whether server status is available from the current runtime.

#### Scenario: MCP servers are configured
- **WHEN** one or more MCP servers are configured
- **THEN** `/doctor` lists their names and reports each server as configured, unavailable, or errored based on available runtime information

#### Scenario: No MCP servers are configured
- **WHEN** no MCP server configuration exists
- **THEN** `/doctor` reports that MCP is not configured without treating it as a failure

### Requirement: `/doctor` SHALL report local tool availability
The diagnostic report SHALL check availability of local executables needed by built-in workflows, including `git` and optionally `gh` for PR review.

#### Scenario: Required executable is available
- **WHEN** `git` is available on `PATH`
- **THEN** `/doctor` marks Git tooling as available

#### Scenario: Optional executable is missing
- **WHEN** `gh` is not available on `PATH`
- **THEN** `/doctor` reports GitHub PR review support as degraded but does not fail the entire report

### Requirement: `/doctor` SHALL validate permission file integrity
The diagnostic report SHALL check whether the permission rules file exists, can be parsed, and contains valid rule entries.

#### Scenario: Permission file is valid
- **WHEN** the permission file exists and parses into valid rules
- **THEN** `/doctor` marks permission configuration as passing

#### Scenario: Permission file is invalid
- **WHEN** the permission file cannot be parsed or contains invalid rule syntax
- **THEN** `/doctor` marks permission configuration as failing and includes a remediation hint
