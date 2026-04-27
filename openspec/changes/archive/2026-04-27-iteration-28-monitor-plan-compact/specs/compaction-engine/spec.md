## ADDED Requirements

### Requirement: Re-inject project guidance after compaction
After successful compaction, the system SHALL preserve project guidance from discovered `CLAUDE.md` content by including a bounded project-guidance section in the compaction context used for continued conversation.

#### Scenario: Project guidance available during compaction
- **WHEN** compaction runs in a project with discovered `CLAUDE.md` content
- **THEN** the compacted conversation context SHALL include a bounded summary or excerpt of that project guidance for subsequent API requests

#### Scenario: Project guidance unavailable during compaction
- **WHEN** no `CLAUDE.md` content is available during compaction
- **THEN** compaction SHALL continue without failing and SHALL omit the project-guidance section

### Requirement: Preserve used MCP tool context after compaction
After successful compaction, the system SHALL preserve a bounded summary of MCP servers and MCP tools that were used during the session when that information is available.

#### Scenario: MCP tools were used before compaction
- **WHEN** compaction runs after one or more MCP tools have been called in the session
- **THEN** the compacted conversation context SHALL include the MCP server names and tool names needed to understand those prior tool interactions

#### Scenario: No MCP tools were used before compaction
- **WHEN** compaction runs without any recorded MCP tool usage
- **THEN** compaction SHALL omit the MCP tool context section

### Requirement: Preserve recent permission decisions after compaction
After successful compaction, the system SHALL preserve a bounded summary of recent permission decisions so the agent can maintain continuity about allowed and denied actions.

#### Scenario: Permission decisions exist before compaction
- **WHEN** recent permission decisions have been recorded before compaction
- **THEN** the compacted conversation context SHALL include a bounded list of recent allow, deny, or ask decisions with associated tool names

#### Scenario: Permission context exceeds bound
- **WHEN** the number of recent permission decisions exceeds the configured bound
- **THEN** compaction SHALL include only the most recent decisions up to the bound
