## ADDED Requirements

### Requirement: AgentTool spawns independent sub-agent
The system SHALL provide an `AgentTool` that spawns an independent QueryLoop sub-agent when invoked by the model. The sub-agent SHALL have its own message history and SHALL return its final text output as the tool result to the parent loop.

#### Scenario: Successful sub-agent execution
- **WHEN** the model invokes `AgentTool` with a prompt "Refactor the error handling in src/lib.rs"
- **THEN** the system SHALL create a new QueryLoop with an empty message history, run it with the given prompt, and return the sub-agent's final assistant text as the tool result

#### Scenario: Sub-agent uses tools
- **WHEN** a sub-agent executes and its internal model calls tools (e.g., FileRead, FileEdit, Bash)
- **THEN** the sub-agent's tool calls SHALL execute normally using the same cwd and permission rules as the parent

### Requirement: AgentTool input schema
The `AgentTool` SHALL accept the following input fields:
- `prompt` (required string): The task description for the sub-agent
- `allowed_tools` (optional array of strings): Tool names the sub-agent is allowed to use. If omitted, the sub-agent SHALL have access to all tools except AgentTool itself at max depth.

#### Scenario: Sub-agent with tool filter
- **WHEN** AgentTool is invoked with `allowed_tools: ["FileRead", "Bash"]`
- **THEN** the sub-agent SHALL only have access to FileRead and Bash tools

#### Scenario: Sub-agent without tool filter
- **WHEN** AgentTool is invoked without `allowed_tools`
- **THEN** the sub-agent SHALL have access to all registered tools (including AgentTool if depth allows)

### Requirement: Sub-agent inherits parent configuration
The sub-agent SHALL inherit the following from the parent state:
- Working directory (cwd)
- Permission mode and permission rules (always_allow, always_deny)
- Model settings (model, max_tokens, stream, thinking)
- System prompt
The sub-agent SHALL NOT inherit the parent's message history.

#### Scenario: Sub-agent inherits cwd and permissions
- **WHEN** the parent is running in `/workspace` with permission mode `AcceptEdits`
- **THEN** the sub-agent SHALL operate in `/workspace` with `AcceptEdits` mode

#### Scenario: Sub-agent has empty message history
- **WHEN** the parent has 20 messages in its conversation history and spawns a sub-agent
- **THEN** the sub-agent SHALL start with zero messages (only the user prompt from the AgentTool invocation)

### Requirement: Recursive depth limiting
The system SHALL enforce a maximum recursion depth for nested AgentTool invocations. The default maximum depth SHALL be 3. When the depth limit is reached, the AgentTool SHALL return an error result instead of spawning a new sub-agent.

#### Scenario: Depth limit reached
- **WHEN** AgentTool is invoked at recursion depth 3 (the default maximum)
- **THEN** the system SHALL return a tool error result indicating the maximum agent nesting depth has been reached

#### Scenario: Nested agents within limit
- **WHEN** a sub-agent at depth 1 invokes AgentTool (creating depth 2)
- **THEN** the system SHALL successfully spawn the nested sub-agent

### Requirement: Sub-agent result includes usage summary
The tool result returned by AgentTool SHALL include the sub-agent's final text output and a summary of token usage (input tokens, output tokens) consumed by the sub-agent session.

#### Scenario: Result format with usage
- **WHEN** a sub-agent completes successfully using 1000 input tokens and 500 output tokens
- **THEN** the tool result SHALL contain the sub-agent's final text followed by a usage summary line

### Requirement: Sub-agent has bounded execution
The sub-agent QueryLoop SHALL have a default maximum of 5 rounds (lower than the parent's default of 8) to prevent runaway execution.

#### Scenario: Sub-agent round limit
- **WHEN** a sub-agent reaches 5 rounds without completing
- **THEN** the sub-agent SHALL stop and return whatever result it has accumulated

### Requirement: AgentTool is non-read-only and non-concurrency-safe
AgentTool SHALL be classified as non-read-only and non-concurrency-safe, ensuring it goes through the permission system and executes serially.

#### Scenario: AgentTool permission check
- **WHEN** permission mode is Default and the model invokes AgentTool
- **THEN** the system SHALL evaluate AgentTool through the same permission check as other non-read-only tools

### Requirement: AgentContext is injected via ToolContext
The system SHALL provide an `AgentContext` structure within `ToolContext` that carries the dependencies needed to spawn a sub-agent: a sub-agent execution callback provided by the CLI layer, a tool registry template or filtering input, and current recursion depth. In this iteration, sub-agents SHALL NOT inherit hook execution from the parent agent.

#### Scenario: AgentContext available when AgentTool registered
- **WHEN** the CLI initializes and registers AgentTool in the ToolRegistry
- **THEN** the ToolContext passed to tool execution SHALL include a populated AgentContext

#### Scenario: AgentContext absent gracefully
- **WHEN** AgentTool is executed in a context without AgentContext (e.g., unit test)
- **THEN** AgentTool SHALL return a tool error indicating agent context is not available
