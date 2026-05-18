## MODIFIED Requirements

### Requirement: Sandbox configuration controls tool isolation
The system SHALL expose sandbox configuration with enabled state, allowed filesystem paths, network policy, and sandbox-aware Bash approval behavior. Sandboxing SHALL be disabled by default and SHALL be configurable through runtime config and CLI overrides.

#### Scenario: Sandbox defaults are disabled
- **WHEN** no CLI option, environment variable, or settings file enables sandboxing
- **THEN** tool execution SHALL preserve existing unsandboxed behavior

#### Scenario: Sandbox settings enable isolation
- **WHEN** effective configuration sets `sandbox.enabled = true`
- **THEN** supported tool executions SHALL use the sandbox runner before accessing host process resources

#### Scenario: Sandbox allowed paths are resolved
- **WHEN** sandbox allowed paths include project-relative, home-relative, or absolute paths
- **THEN** the system SHALL canonicalize those paths before enforcing filesystem access boundaries

#### Scenario: CLI enables sandboxing
- **WHEN** the CLI starts with `--sandbox`
- **THEN** the effective sandbox configuration SHALL set `sandbox.enabled = true`

#### Scenario: CLI disables sandbox network access
- **WHEN** the CLI starts with `--sandbox-no-network`
- **THEN** the effective sandbox configuration SHALL set the sandbox network policy to deny outbound network access for sandboxed tools

### Requirement: Sandboxed Bash execution restricts filesystem access
The system SHALL execute Bash commands inside an OS sandbox when sandboxing is enabled and SHALL restrict file access to configured allowed paths. If sandboxing is explicitly enabled but no supported sandbox runtime is available, Bash SHALL fail closed with a clear sandbox unsupported error.

#### Scenario: Bash cannot read outside allowed paths
- **WHEN** sandboxing is enabled with only the project root allowed and Bash attempts to read a file outside that root
- **THEN** the command SHALL fail due to sandbox restrictions rather than reading the external file

#### Scenario: Bash can read inside allowed paths
- **WHEN** sandboxing is enabled and Bash reads a file under an allowed path
- **THEN** the command SHALL run successfully subject to normal command exit status

#### Scenario: Unsupported sandbox runtime is reported
- **WHEN** sandboxing is enabled on a platform or host without a supported sandbox runtime
- **THEN** the tool result SHALL report a clear sandbox unsupported error and MUST NOT silently run unsandboxed

#### Scenario: Existing Bash behavior is preserved inside sandbox
- **WHEN** a sandboxed Bash command completes successfully and changes directory before exiting
- **THEN** the tool SHALL still capture the final working directory, update session CWD when valid, enforce timeout, and truncate long output using the existing Bash behavior

### Requirement: Sandbox network policy applies to sandboxed tools
The system SHALL apply sandbox network policy to sandboxed tool executions without blocking the model API client or other non-sandboxed runtime services.

#### Scenario: Network disabled blocks sandboxed outbound access
- **WHEN** sandboxing is enabled with network disabled and Bash attempts outbound network access
- **THEN** the sandboxed command SHALL be blocked by the sandbox network policy

#### Scenario: Network policy does not block model requests
- **WHEN** sandboxing is enabled with network disabled for tools
- **THEN** the CLI SHALL still be able to send model API requests outside sandboxed tool execution

#### Scenario: Network allowed preserves outbound behavior
- **WHEN** sandboxing is enabled with network policy set to allow
- **THEN** sandboxed Bash commands SHALL NOT be blocked by sandbox network policy solely because they attempt outbound network access

## ADDED Requirements

### Requirement: Sandbox adapters are selected by platform
The system SHALL provide platform-specific sandbox adapters and select the active adapter at runtime based on host platform and runtime availability.

#### Scenario: macOS adapter is available
- **WHEN** the host platform is macOS and `sandbox-exec` is available
- **THEN** sandboxed Bash execution SHALL use a generated `sandbox-exec` profile for filesystem and network policy enforcement

#### Scenario: Linux adapter is available
- **WHEN** the host platform is Linux and `bwrap` is available
- **THEN** sandboxed Bash execution SHALL use `bwrap` arguments for filesystem bindings and network policy enforcement

#### Scenario: No adapter is available
- **WHEN** sandboxing is enabled and no platform adapter is available
- **THEN** sandboxed Bash execution SHALL return a sandbox unsupported error before running the requested command

### Requirement: Sandbox configuration is available to tools through shared state
The system SHALL store effective sandbox configuration in shared application/session state so tools can apply sandbox behavior without changing their input schemas.

#### Scenario: Bash reads sandbox config from tool context
- **WHEN** Bash executes with a `ToolContext` containing application state
- **THEN** Bash SHALL use the effective sandbox configuration from that state to decide whether and how to wrap the command

#### Scenario: Bash has no application state
- **WHEN** Bash executes without application state in its `ToolContext`
- **THEN** Bash SHALL preserve existing unsandboxed behavior unless a future direct sandbox context is explicitly supplied
