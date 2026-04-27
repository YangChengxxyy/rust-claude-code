## ADDED Requirements

### Requirement: The system SHALL define a typed memory taxonomy
The memory subsystem SHALL classify durable memory into the four reference-aligned types `user`, `feedback`, `project`, and `reference`.

#### Scenario: Parse a valid memory type
- **WHEN** a memory file frontmatter contains a `type` field set to `user`, `feedback`, `project`, or `reference`
- **THEN** the system recognizes that type as valid typed memory metadata

#### Scenario: Handle a missing or unknown memory type
- **WHEN** a memory file has no `type` field or contains an unknown type value
- **THEN** the system degrades gracefully without failing the memory load

### Requirement: The system SHALL inject memory behavior rules into prompt construction
The system SHALL provide explicit memory behavior guidance covering what memory is for, what should be saved, what should not be saved, when memory should be accessed, and how recalled memory should be treated.

#### Scenario: Build prompt with memory contract
- **WHEN** the system constructs prompt context for a session with memory enabled
- **THEN** it includes memory contract guidance that explains the meaning and intended use of each memory type

#### Scenario: Include no-save guidance
- **WHEN** the system constructs the memory contract prompt section
- **THEN** it includes guidance that excludes derivable repo state, git history, transient task state, secrets, and other non-durable context from memory

#### Scenario: Include auto-memory save guidance
- **WHEN** the system constructs prompt context for a session with automatic memory enabled
- **THEN** it includes guidance to save durable corrections, stable preferences, and non-derivable project context through the memory write path

#### Scenario: Include auto-memory disabled guidance
- **WHEN** automatic memory is disabled by environment configuration
- **THEN** the prompt guidance does not ask the agent to create automatic memory writes

### Requirement: The system SHALL define ignore-memory semantics
The system SHALL support an explicit "do not use memory" behavior in which memory is treated as unavailable for the current response.

#### Scenario: User asks to ignore memory
- **WHEN** the user explicitly says to ignore or not use memory
- **THEN** the system behaves as if no memory content were available for that response

### Requirement: The system SHALL define verification-oriented recall semantics
The system SHALL instruct the model to treat recalled memory as historical context that may need verification before being used as current fact.

#### Scenario: Memory references a file, function, or flag
- **WHEN** recalled memory mentions a concrete file path, function name, or flag
- **THEN** the system requires that information to be treated as something to verify rather than assert as current truth

### Requirement: The memory contract SHALL distinguish automatic and manual memory behavior
The memory contract SHALL state that automatic memory writes are opportunistic and policy-gated, while explicit user memory commands remain authoritative.

#### Scenario: User explicitly asks to remember
- **WHEN** the user explicitly asks the system to remember a detail
- **THEN** the system treats it as a manual memory intent even if automatic memory is disabled

#### Scenario: Agent infers a memory candidate
- **WHEN** the agent infers a memory candidate without an explicit user command
- **THEN** the system treats it as automatic memory and applies automatic-memory policy checks
