## MODIFIED Requirements

### Requirement: The system SHALL inject memory behavior rules into prompt construction
The system SHALL provide explicit memory behavior guidance covering what memory is for, what should be saved, what should not be saved, when memory should be accessed, how recalled memory should be treated, and when automatic memory writes are allowed.

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

## ADDED Requirements

### Requirement: The memory contract SHALL distinguish automatic and manual memory behavior
The memory contract SHALL state that automatic memory writes are opportunistic and policy-gated, while explicit user memory commands remain authoritative.

#### Scenario: User explicitly asks to remember
- **WHEN** the user explicitly asks the system to remember a detail
- **THEN** the system treats it as a manual memory intent even if automatic memory is disabled

#### Scenario: Agent infers a memory candidate
- **WHEN** the agent infers a memory candidate without an explicit user command
- **THEN** the system treats it as automatic memory and applies automatic-memory policy checks
