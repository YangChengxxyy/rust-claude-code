## ADDED Requirements

### Requirement: Versioned config migration runner
The system SHALL provide a migration runner that reads a persisted JSON config file, determines its `_migration_version`, runs all registered migrations with a greater version in ascending order, and writes the upgraded config back with the latest migration version.

#### Scenario: Run pending migrations
- **WHEN** a config file has `_migration_version: 1` and migrations for versions 2 and 3 are registered
- **THEN** the runner SHALL run versions 2 and 3 in order and write `_migration_version: 3`

#### Scenario: No config file exists
- **WHEN** the migration runner is invoked for a missing config file
- **THEN** the runner SHALL complete without creating a config file

### Requirement: Migration failure stops upgrade
The system SHALL stop running migrations and return an error if any migration fails.

#### Scenario: Migration returns error
- **WHEN** a registered migration returns an error
- **THEN** the runner SHALL not run later migrations and SHALL report the failure

### Requirement: Default config migrations
The system SHALL include a default migration set with a baseline migration and a model rename migration that updates `model: "claude-3-opus"` to the configured Opus 4 replacement.

#### Scenario: Legacy Opus model is renamed
- **WHEN** a config file contains `model: "claude-3-opus"` before the model rename migration runs
- **THEN** the migrated config SHALL contain the replacement Opus 4 model name

### Requirement: Config loading runs migrations before deserialization
The system SHALL run pending migrations for the persisted config file before deserializing it into the typed runtime configuration.

#### Scenario: Startup loads old config
- **WHEN** the CLI starts with an old persisted config file
- **THEN** pending migrations SHALL complete before the typed config is used
