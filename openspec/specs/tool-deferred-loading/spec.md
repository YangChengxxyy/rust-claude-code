## Purpose

Define deferred tool loading behavior so infrequently used tools can be discovered on demand without sending every full schema in every API request.

## Requirements

### Requirement: Tool trait supports deferred loading declaration
The system SHALL extend the `Tool` trait with a `should_defer(&self) -> bool` method. The default implementation SHALL return `false`. When a tool returns `true`, the system SHALL treat it as a deferred tool.

#### Scenario: Default tool is not deferred
- **WHEN** a tool implements `Tool` without overriding `should_defer`
- **THEN** `should_defer()` SHALL return `false`

#### Scenario: Deferred tool declares itself
- **WHEN** `WebSearchTool::should_defer()` is called
- **THEN** it SHALL return `true`

### Requirement: Deferred tools send minimal definition in API requests
The system SHALL represent deferred tools in API requests with only their `name` and a `deferred: true` flag. The `description` and `input_schema` fields SHALL be omitted for deferred tools.

#### Scenario: Non-deferred tool sends full schema
- **WHEN** `BashTool` (non-deferred) is included in an API request
- **THEN** the serialized tool object SHALL contain `name`, `description`, and `input_schema`

#### Scenario: Deferred tool sends minimal definition
- **WHEN** `WebSearchTool` (deferred) is included in an API request
- **THEN** the serialized tool object SHALL contain only `name` and `deferred: true`, omitting `description` and `input_schema`

#### Scenario: Backward compatibility for manually constructed ApiTool
- **WHEN** code constructs `ApiTool::new(name, description, input_schema)` without setting `deferred`
- **THEN** the serialized tool object SHALL contain `name`, `description`, and `input_schema` (no `deferred` field)

### Requirement: ToolRegistry exposes deferred and non-deferred tool partitions
The `ToolRegistry` SHALL provide methods to query deferred and non-deferred tools separately:
- `get_deferred_tools()` SHALL return all tools where `should_defer() == true`
- `get_non_deferred_tools()` SHALL return all tools where `should_defer() == false`
- `search_tools(query: &str, max: usize)` SHALL search deferred tools by name and description
- `estimate_deferred_schema_tokens()` SHALL return an estimated token count of all deferred tools' schemas combined
The behavior of these methods SHALL be covered by registry contract tests.

#### Scenario: Registry partitions tools correctly
- **WHEN** a registry contains `BashTool` (non-deferred) and `WebSearchTool` (deferred)
- **THEN** `get_non_deferred_tools()` SHALL contain only `BashTool`
- **THEN** `get_deferred_tools()` SHALL contain only `WebSearchTool`

#### Scenario: Empty registry returns empty partitions
- **WHEN** `ToolRegistry` is empty
- **THEN** `get_deferred_tools()` and `get_non_deferred_tools()` SHALL both return empty vectors

#### Scenario: Deferred schema search is contract tested
- **WHEN** registry tests search for a deferred tool by exact selection
- **THEN** the result SHALL include that deferred tool's full schema information
- **THEN** the same search SHALL exclude non-deferred tools

### Requirement: QueryLoop tracks discovered deferred tools
The `QueryLoop` SHALL maintain a `discovered_tools: HashSet<String>` field. When a deferred tool is actually invoked by the model and executed, its name SHALL be added to `discovered_tools`. In each subsequent API request, any tool in `discovered_tools` SHALL be sent with its full schema regardless of its `should_defer()` value.

#### Scenario: Deferred tool discovered after first use
- **WHEN** the model invokes `WebSearchTool` for the first time
- **THEN** `"WebSearch"` SHALL be added to `discovered_tools`
- **THEN** in the next API request, `WebSearchTool` SHALL be sent with full `description` and `input_schema`

#### Scenario: Undiscovered deferred tool remains minimal
- **WHEN** `WebSearchTool` is registered and deferred but never invoked
- **THEN** in every API request, it SHALL be sent with only `name` and `deferred: true`

#### Scenario: Discovery state persists across turns
- **WHEN** `WebSearchTool` is discovered in turn 3
- **THEN** in turns 4, 5, and beyond, it SHALL continue to be sent with full schema

### Requirement: Automatic threshold controls deferred loading activation
The system SHALL only enable deferred loading when the estimated token savings are significant. The threshold SHALL be: deferred tools' combined schema token estimate exceeds 10% of the model's context window size. If the threshold is not met, all tools SHALL be sent with full schema.

#### Scenario: Many deferred tools exceed threshold
- **WHEN** 20 MCP tools with large schemas are registered and their combined estimated tokens exceed 20K (10% of 200K context window)
- **THEN** deferred loading SHALL be active and only non-deferred tools send full schema

#### Scenario: Few deferred tools below threshold
- **WHEN** only 2 small deferred tools are registered with combined estimated tokens of 500
- **THEN** deferred loading SHALL NOT be active and all tools SHALL send full schema

#### Scenario: Threshold disables deferred loading mid-session
- **WHEN** a user removes MCP servers, reducing deferred tool count below threshold
- **THEN** in the next turn, all remaining tools SHALL send full schema

### Requirement: Core tools are never deferred
The following essential tools SHALL never be marked as deferred: `Bash`, `FileRead`, `FileEdit`, `FileWrite`, `Grep`, `Glob`, `TodoWrite`, `EnterPlanMode`, `ExitPlanMode`. These tools SHALL always be sent with full schema in every request.

#### Scenario: Core tools always send full schema
- **WHEN** any of the core tools are registered
- **THEN** `should_defer()` SHALL return `false` for each of them
- **THEN** they SHALL always appear in API requests with complete `description` and `input_schema`

### Requirement: ToolSearchTool itself is never deferred
The `ToolSearchTool` SHALL return `false` from `should_defer()` to ensure the model can always discover other deferred tools.

#### Scenario: Search tool is always available
- **WHEN** deferred loading is active
- **THEN** `ToolSearchTool` SHALL be included in the non-deferred tool list with full schema
