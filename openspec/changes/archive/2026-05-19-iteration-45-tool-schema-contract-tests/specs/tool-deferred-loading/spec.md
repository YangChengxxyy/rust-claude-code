## MODIFIED Requirements

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
