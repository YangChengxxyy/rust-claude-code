## ADDED Requirements

### Requirement: Permission rules support path-glob patterns
The system SHALL support an optional path pattern in permission rules. When a rule includes a path pattern, the system SHALL match the tool invocation's target file path against the pattern using glob syntax with `*` (single path segment) and `**` (recursive) wildcards.

#### Scenario: Allow edits to TypeScript files under src
- **WHEN** a permission rule `FileEdit(, /src/**/*.ts)` with type Allow exists and the model invokes FileEdit on `src/components/Header.ts`
- **THEN** the permission check SHALL return Allowed

#### Scenario: Deny reads of .env files
- **WHEN** a permission rule `FileRead(, /.env)` with type Deny exists and the model invokes FileRead on `.env`
- **THEN** the permission check SHALL return Denied

#### Scenario: Rule without path pattern matches all paths
- **WHEN** a permission rule `FileEdit` with type Allow exists and no path pattern is specified
- **THEN** the rule SHALL match any FileEdit invocation regardless of target file path

#### Scenario: Rule with both command and path pattern
- **WHEN** a permission rule `Bash(npm *, /package.json)` exists
- **THEN** the rule SHALL only match Bash invocations where the command starts with `npm ` AND the context involves `package.json`

### Requirement: Path pattern prefix resolution
The system SHALL interpret path pattern prefixes as follows: `/path` resolves relative to the project root (git root), `./path` resolves relative to the session CWD, `~/path` resolves relative to the user home directory, and `//path` resolves as an absolute filesystem path. Patterns without a recognized prefix SHALL be treated as project-root-relative.

#### Scenario: Project-root-relative path pattern
- **WHEN** the project root is `/repo` and a rule has path pattern `/src/**/*.rs`
- **THEN** the pattern SHALL match files like `/repo/src/main.rs` and `/repo/src/lib/utils.rs`

#### Scenario: CWD-relative path pattern
- **WHEN** the session CWD is `/repo/frontend` and a rule has path pattern `./components/**`
- **THEN** the pattern SHALL match files like `/repo/frontend/components/App.tsx` but NOT `/repo/backend/components/App.tsx`

#### Scenario: Home-relative path pattern
- **WHEN** the user home is `/home/user` and a rule has path pattern `~/secrets/*`
- **THEN** the pattern SHALL match files like `/home/user/secrets/key.pem`

#### Scenario: Absolute path pattern
- **WHEN** a rule has path pattern `///etc/hosts`
- **THEN** the pattern SHALL match exactly `/etc/hosts`

### Requirement: Three-level rule evaluation with Ask tier
The system SHALL evaluate permission rules in three tiers: Deny rules first, then Ask rules, then Allow rules, then the mode-specific default. When an Ask rule matches, the result SHALL be `NeedsConfirmation`. `BypassPermissions` mode SHALL override Ask rules to Allowed. `Plan` mode SHALL override Ask rules to Denied for non-read-only tools.

#### Scenario: Ask rule forces confirmation
- **WHEN** permission mode is `Default`, an Ask rule matches the tool invocation, and no Deny rule matches
- **THEN** the permission check SHALL return `NeedsConfirmation`

#### Scenario: Deny rule takes precedence over Ask rule
- **WHEN** both a Deny rule and an Ask rule match the same tool invocation
- **THEN** the permission check SHALL return Denied

#### Scenario: Ask rule takes precedence over Allow rule
- **WHEN** both an Ask rule and an Allow rule match the same tool invocation
- **THEN** the permission check SHALL return `NeedsConfirmation`

#### Scenario: BypassPermissions overrides Ask to Allowed
- **WHEN** permission mode is `BypassPermissions` and an Ask rule matches
- **THEN** the permission check SHALL return Allowed

#### Scenario: Plan mode overrides Ask to Denied for writes
- **WHEN** permission mode is `Plan` and an Ask rule matches a non-read-only tool
- **THEN** the permission check SHALL return Denied

### Requirement: Auto-extract file paths from tool inputs
The system SHALL extract the target file path from `FileEdit`, `FileWrite`, and `FileRead` tool inputs for use in path-based permission rule matching. The extracted path SHALL be resolved against the session CWD before matching.

#### Scenario: FileEdit path extraction
- **WHEN** the model invokes FileEdit with `file_path = "src/main.rs"` and the session CWD is `/repo`
- **THEN** the permission system SHALL extract and resolve the path to `/repo/src/main.rs` for rule matching

#### Scenario: FileWrite path extraction
- **WHEN** the model invokes FileWrite with `file_path = "/tmp/output.txt"`
- **THEN** the permission system SHALL use `/tmp/output.txt` as the resolved path for rule matching

#### Scenario: Bash tool does not extract paths
- **WHEN** the model invokes Bash with a command containing file paths
- **THEN** the permission system SHALL NOT attempt to extract file paths from the Bash command; command-prefix matching remains the mechanism for Bash

### Requirement: Compact string syntax for path rules
The system SHALL support a compact string representation for rules with path patterns. The format SHALL be `Tool(command_pattern, path_pattern)` for rules with both patterns, `Tool(, path_pattern)` for path-only rules, and the existing `Tool(command_pattern)` for command-only rules. Parsing and serialization SHALL be round-trip consistent.

#### Scenario: Parse path-only rule
- **WHEN** the string `"FileEdit(, /src/**/*.ts)"` is parsed as an Allow rule
- **THEN** the result SHALL be a rule with tool_name `FileEdit`, no command pattern, and path pattern `/src/**/*.ts`

#### Scenario: Parse rule with both patterns
- **WHEN** the string `"Bash(git *, /repo/**)"` is parsed as a Deny rule
- **THEN** the result SHALL be a rule with tool_name `Bash`, command pattern `git *`, and path pattern `/repo/**`

#### Scenario: Serialize and re-parse round-trip
- **WHEN** a rule with tool_name `FileRead`, no command pattern, and path pattern `~/.ssh/*` is serialized to compact form and then parsed back
- **THEN** the re-parsed rule SHALL be identical to the original
