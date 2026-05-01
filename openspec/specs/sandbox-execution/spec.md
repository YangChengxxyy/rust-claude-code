## ADDED Requirements

### Requirement: Sandbox configuration controls tool isolation
The system SHALL expose sandbox configuration with enabled state, allowed filesystem paths, network policy, and sandbox-aware Bash approval behavior.

#### Scenario: Sandbox defaults are disabled
- **WHEN** no CLI option, environment variable, or settings file enables sandboxing
- **THEN** tool execution SHALL preserve existing unsandboxed behavior

#### Scenario: Sandbox settings enable isolation
- **WHEN** effective configuration sets `sandbox.enabled = true`
- **THEN** supported tool executions SHALL use the sandbox runner before accessing host process resources

#### Scenario: Sandbox allowed paths are resolved
- **WHEN** sandbox allowed paths include project-relative, home-relative, or absolute paths
- **THEN** the system SHALL canonicalize those paths before enforcing filesystem access boundaries

### Requirement: Sandboxed Bash execution restricts filesystem access
The system SHALL execute Bash commands inside an OS sandbox when sandboxing is enabled and SHALL restrict file access to configured allowed paths.

#### Scenario: Bash cannot read outside allowed paths
- **WHEN** sandboxing is enabled with only the project root allowed and Bash attempts to read a file outside that root
- **THEN** the command SHALL fail due to sandbox restrictions rather than reading the external file

#### Scenario: Bash can read inside allowed paths
- **WHEN** sandboxing is enabled and Bash reads a file under an allowed path
- **THEN** the command SHALL run successfully subject to normal command exit status

#### Scenario: Unsupported sandbox runtime is reported
- **WHEN** sandboxing is enabled on a platform or host without a supported sandbox runtime
- **THEN** the tool result SHALL report a clear sandbox unsupported error and MUST NOT silently run unsandboxed

### Requirement: Sandbox network policy applies to sandboxed tools
The system SHALL apply sandbox network policy to sandboxed tool executions without blocking the model API client or other non-sandboxed runtime services.

#### Scenario: Network disabled blocks sandboxed outbound access
- **WHEN** sandboxing is enabled with network disabled and Bash attempts outbound network access
- **THEN** the sandboxed command SHALL be blocked by the sandbox network policy

#### Scenario: Network policy does not block model requests
- **WHEN** sandboxing is enabled with network disabled for tools
- **THEN** the CLI SHALL still be able to send model API requests outside sandboxed tool execution

### Requirement: Sandbox-aware Bash auto-approval is configurable
The system SHALL support `autoAllowBashIfSandboxed` so Bash commands running inside an active sandbox can skip normal confirmation when policy allows it.

#### Scenario: Sandboxed Bash is auto-approved
- **WHEN** sandboxing is active and `autoAllowBashIfSandboxed` is enabled
- **THEN** Bash permission checks SHALL allow the command without prompting unless an explicit deny rule or Auto safety failure applies

#### Scenario: Unsandboxed Bash is not auto-approved by sandbox setting
- **WHEN** sandboxing is disabled and `autoAllowBashIfSandboxed` is enabled
- **THEN** Bash permission checks SHALL use the normal permission mode behavior
