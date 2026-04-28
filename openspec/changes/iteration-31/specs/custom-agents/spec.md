## ADDED Requirements

### Requirement: Custom agent discovery
The system SHALL discover custom agent definition files from the project `.claude/agents/` directory. Each regular Markdown file with extension `.md` in that directory SHALL be considered a candidate agent definition.

#### Scenario: Discover project custom agents
- **WHEN** the current project contains `.claude/agents/reviewer.md`
- **THEN** the custom agent loader SHALL include `reviewer.md` as an agent definition candidate

#### Scenario: Missing agents directory
- **WHEN** the current project has no `.claude/agents/` directory
- **THEN** custom agent discovery SHALL return an empty registry without error

### Requirement: Custom agent definition format
Each custom agent definition SHALL use Markdown with YAML front matter. The front matter SHALL include `name` and `description`, MAY include `tools` as an array of tool names, and MAY include `model` as a string. The Markdown body after front matter SHALL be used as the agent `system_prompt`.

#### Scenario: Parse valid custom agent
- **WHEN** `.claude/agents/reviewer.md` contains front matter with `name: reviewer`, `description: Reviews code`, `tools: [FileRead, Bash]`, and a Markdown body
- **THEN** the loader SHALL create a custom agent named `reviewer` with the provided description, tool allowlist, and system prompt body

#### Scenario: Parse optional model
- **WHEN** a custom agent definition contains `model: claude-3-5-sonnet-latest`
- **THEN** the loaded agent SHALL include that model override

#### Scenario: Missing required field
- **WHEN** a custom agent definition is missing `name` or `description`
- **THEN** the loader SHALL reject that definition and report a validation error without rejecting other valid agent definitions

### Requirement: Custom agent naming
Custom agent names SHALL be unique within the loaded registry and SHALL use a kebab-case identifier suitable for AgentTool input.

#### Scenario: Duplicate agent name
- **WHEN** two custom agent definition files declare the same `name`
- **THEN** the loader SHALL keep one deterministic entry and report a duplicate-name validation error

#### Scenario: Invalid agent name
- **WHEN** a custom agent definition declares `name: "Code Reviewer!"`
- **THEN** the loader SHALL reject that definition because the name is not kebab-case

### Requirement: Custom agent registry lookup
The system SHALL provide a custom agent registry that supports listing all loaded agents and looking up an agent by name.

#### Scenario: List loaded agents
- **WHEN** two valid custom agents named `reviewer` and `tester` are loaded
- **THEN** the registry SHALL list both agents with their names and descriptions

#### Scenario: Lookup loaded agent
- **WHEN** AgentTool requests custom agent `reviewer`
- **THEN** the registry SHALL return the `reviewer` definition

#### Scenario: Lookup missing agent
- **WHEN** AgentTool requests custom agent `missing-agent`
- **THEN** the registry SHALL report that no custom agent with that name exists
