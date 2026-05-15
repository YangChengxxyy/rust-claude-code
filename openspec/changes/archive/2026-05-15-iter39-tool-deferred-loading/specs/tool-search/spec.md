## ADDED Requirements

### Requirement: ToolSearchTool provides model-side tool discovery
The system SHALL provide a `ToolSearch` tool that allows the model to search for and discover deferred tools by name or keyword. The tool SHALL be read-only and SHALL never be deferred.

#### Scenario: ToolSearchTool is registered
- **WHEN** the system initializes the `ToolRegistry`
- **THEN** `ToolSearchTool` SHALL be registered with name `ToolSearch`

#### Scenario: ToolSearchTool is read-only
- **WHEN** `ToolSearchTool::is_read_only()` is queried
- **THEN** it SHALL return `true`

### Requirement: ToolSearchTool input schema
The `ToolSearch` tool SHALL accept the following input fields:
- `query` (required string): The search query, supporting plain keywords or `select:ToolName` exact selection syntax
- `max_results` (optional integer): Maximum number of results to return. Default SHALL be 5. Maximum SHALL be 20.

#### Scenario: Search with keyword query
- **WHEN** `ToolSearch` is invoked with `query: "web"`
- **THEN** it SHALL search deferred tools matching "web" and return up to 5 results

#### Scenario: Search with exact selection
- **WHEN** `ToolSearch` is invoked with `query: "select:WebSearchTool"`
- **THEN** it SHALL return only the `WebSearchTool` schema (if it exists and is deferred)

#### Scenario: Search with custom max_results
- **WHEN** `ToolSearch` is invoked with `query: "read"` and `max_results: 3`
- **THEN** it SHALL return at most 3 matching tool schemas

### Requirement: ToolSearchTool search logic
The `ToolSearchTool` SHALL search only deferred tools. The search logic SHALL support:
1. Exact selection: queries starting with `select:` SHALL perform case-insensitive exact name match on the tool name following the prefix
2. Keyword search: for non-select queries, split the query into keywords and match against tool names and descriptions
3. Name tokenization: tool names SHALL be split on CamelCase boundaries and `__` (MCP name separator) for matching

#### Scenario: Exact selection finds tool
- **WHEN** `ToolSearch` is invoked with `query: "select:WebSearchTool"`
- **THEN** it SHALL return `WebSearchTool`'s full schema

#### Scenario: Exact selection misses non-deferred tool
- **WHEN** `ToolSearch` is invoked with `query: "select:Bash"`
- **THEN** it SHALL return no results because `Bash` is not deferred

#### Scenario: Keyword search matches name
- **WHEN** `ToolSearch` is invoked with `query: "search"`
- **THEN** it SHALL match `WebSearchTool` because "search" matches a token in its name

#### Scenario: Keyword search matches description
- **WHEN** `ToolSearch` is invoked with `query: "notebook"`
- **THEN** it SHALL match `NotebookEditTool` because "notebook" appears in its description

#### Scenario: CamelCase name splitting
- **WHEN** `ToolSearch` is invoked with `query: "edit"`
- **THEN** it SHALL match `NotebookEditTool` because "Edit" is a CamelCase token

#### Scenario: MCP name splitting
- **WHEN** `ToolSearch` is invoked with `query: "lookup"`
- **THEN** it SHALL match `mcp__remote__lookup` because "lookup" is a token after `__` split

#### Scenario: No matches returns empty list
- **WHEN** `ToolSearch` is invoked with `query: "nonexistent_xyz"`
- **THEN** it SHALL return an empty list of results

### Requirement: ToolSearchTool scoring and ranking
Search results SHALL be ranked by relevance score. The scoring SHALL use:
- Name match weight: 2x
- Description match weight: 1x
Results SHALL be returned in descending score order. If scores are equal, alphabetical order by tool name SHALL be used as a tiebreaker.

#### Scenario: Name match ranks higher than description match
- **WHEN** `ToolSearch` is invoked with `query: "search"`
- **THEN** `WebSearchTool` (name match) SHALL rank above a tool whose description merely mentions "search"

#### Scenario: Multiple keyword matches increase score
- **WHEN** `ToolSearch` is invoked with `query: "web search"`
- **THEN** `WebSearchTool` SHALL have a higher score than a tool matching only "web" or only "search"

### Requirement: ToolSearchTool output format
The `ToolSearchTool` SHALL return a JSON array of matching tool definitions. Each entry SHALL contain the tool's `name`, `description`, and `input_schema`. The array length SHALL not exceed `max_results`.

#### Scenario: Single result output
- **WHEN** `ToolSearch` returns one match
- **THEN** the result SHALL be a JSON array with one object containing `name`, `description`, and `input_schema`

#### Scenario: Multiple results output
- **WHEN** `ToolSearch` returns three matches
- **THEN** the result SHALL be a JSON array of three objects, ordered by relevance score

#### Scenario: Empty results output
- **WHEN** `ToolSearch` finds no matches
- **THEN** the result SHALL be an empty JSON array `[]`

### Requirement: Discovered tools are automatically promoted to non-deferred
When the model successfully uses `ToolSearchTool` to discover a deferred tool and subsequently invokes that tool in a later turn, the discovered tool SHALL be automatically promoted to non-deferred status for the remainder of the session.

#### Scenario: Search then use promotes tool
- **WHEN** the model uses `ToolSearch` to find `WebSearchTool`
- **AND THEN** the model invokes `WebSearchTool` in a subsequent turn
- **THEN** in the turn after invocation, `WebSearchTool` SHALL be sent with full schema even though it was originally deferred
