## ADDED Requirements

### Requirement: AgentTool named custom agent input
The `AgentTool` input schema SHALL accept an optional `agent` string field naming a loaded custom agent. When `agent` is omitted, AgentTool SHALL preserve existing ad hoc sub-agent behavior.

#### Scenario: AgentTool called with custom agent
- **WHEN** AgentTool is invoked with `agent: "reviewer"` and `prompt: "Review this diff"`
- **THEN** AgentTool SHALL attempt to resolve the custom agent named `reviewer`

#### Scenario: AgentTool called without custom agent
- **WHEN** AgentTool is invoked with only `prompt: "Investigate this bug"`
- **THEN** AgentTool SHALL spawn an ad hoc sub-agent using existing behavior

### Requirement: Custom agent runtime configuration
When AgentTool invokes a named custom agent, the spawned sub-agent SHALL use the custom agent's system prompt, tool allowlist, and optional model override while still inheriting parent cwd, permissions, and bounded execution controls.

#### Scenario: Custom agent system prompt applied
- **WHEN** the `reviewer` custom agent has system prompt `You review code carefully`
- **THEN** the spawned sub-agent SHALL use that system prompt for its model requests

#### Scenario: Custom agent model override applied
- **WHEN** the `reviewer` custom agent declares a model override
- **THEN** the spawned sub-agent SHALL use that model instead of the parent model for its model requests

#### Scenario: Custom agent inherits permissions
- **WHEN** the parent runs in `AcceptEdits` mode and invokes a named custom agent
- **THEN** the spawned sub-agent SHALL run with `AcceptEdits` mode and the same permission rules

#### Scenario: Named custom agent preserves nested delegation
- **WHEN** a named custom agent invokes AgentTool again while the current depth is still below the configured maximum
- **THEN** the nested sub-agent SHALL be spawned successfully with incremented depth and the same bounded recursion controls

### Requirement: Custom agent tool restrictions
When a custom agent declares a `tools` allowlist, the spawned sub-agent SHALL only receive tools from that allowlist. If AgentTool input also includes `allowed_tools`, the effective tool set SHALL be the intersection of the custom agent allowlist and the explicit `allowed_tools` list.

#### Scenario: Agent tool allowlist applied
- **WHEN** custom agent `reviewer` declares `tools: [FileRead, Bash]`
- **THEN** its spawned sub-agent SHALL only have access to FileRead and Bash tools

#### Scenario: Explicit allowed tools further restrict custom agent
- **WHEN** custom agent `reviewer` declares `tools: [FileRead, Bash]` and AgentTool input includes `allowed_tools: [FileRead]`
- **THEN** the spawned sub-agent SHALL only have access to FileRead

#### Scenario: Explicit allowed tools cannot broaden custom agent
- **WHEN** custom agent `reviewer` declares `tools: [FileRead]` and AgentTool input includes `allowed_tools: [FileRead, Bash]`
- **THEN** the spawned sub-agent SHALL only have access to FileRead

### Requirement: Missing custom agent error
If AgentTool input names a custom agent that is not loaded, AgentTool SHALL return a tool error result instead of spawning a sub-agent.

#### Scenario: Missing custom agent requested
- **WHEN** AgentTool is invoked with `agent: "missing-agent"`
- **THEN** AgentTool SHALL return an error result indicating the custom agent was not found
