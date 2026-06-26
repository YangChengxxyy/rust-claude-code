## ADDED Requirements

### Requirement: Configure maximum session budget
The system SHALL support an optional `max_budget_usd` runtime configuration field. The field SHALL follow the same configuration precedence rules as other runtime settings, with higher-precedence sources overriding lower-precedence sources.

#### Scenario: Config file defines budget
- **WHEN** the rust-claude config file contains `max_budget_usd = 1.0`
- **THEN** the effective runtime configuration SHALL set the maximum session budget to `1.0` USD

#### Scenario: Environment overrides config budget
- **WHEN** the config file contains `max_budget_usd = 1.0` and the environment defines `RUST_CLAUDE_MAX_BUDGET_USD = 2.0`
- **THEN** the effective runtime configuration SHALL set the maximum session budget to `2.0` USD

### Requirement: Run config migrations before applying settings
The system SHALL run pending migrations for the selected rust-claude config file before deserializing typed configuration and before merging runtime overrides.

#### Scenario: Migration runs before config load
- **WHEN** the config file contains a value that is changed by a pending migration
- **THEN** the effective runtime configuration SHALL be derived from the migrated config content

#### Scenario: Migration failure blocks unsafe config use
- **WHEN** config migration fails
- **THEN** the system SHALL report the migration failure and MUST NOT continue using a partially migrated config
