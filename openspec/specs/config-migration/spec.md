# config-migration Specification

## Purpose
TBD - created by archiving change implement-cost-migration-budget. Update Purpose after archive.
## Requirements
### Requirement: Versioned config migrations
The system SHALL provide a migration runner for persisted JSON config files. The runner SHALL read `_migration_version`, execute pending migrations in ascending version order, and write the final `_migration_version` after successful migration.

#### Scenario: Baseline config receives migration version
- **WHEN** a config file has no `_migration_version`
- **THEN** the migration runner SHALL treat it as version 0 and write the latest migration version after all pending migrations succeed

#### Scenario: Up-to-date config is unchanged
- **WHEN** a config file already has the latest `_migration_version`
- **THEN** the migration runner SHALL not modify the file contents

### Requirement: Migrations preserve unknown fields
The migration runner SHALL operate on raw JSON values and preserve unknown config fields unless a migration explicitly changes them.

#### Scenario: Unknown field survives migration
- **WHEN** a config file contains an unknown field and a pending migration runs
- **THEN** the unknown field SHALL remain present with the same value after migration

### Requirement: Config migration errors are safe
The migration runner SHALL avoid partially overwriting the original config file when a migration fails or the migrated JSON cannot be written completely.

#### Scenario: Failed migration leaves original file intact
- **WHEN** a pending migration returns an error
- **THEN** the original config file SHALL remain unchanged and startup SHALL report the failed migration version

#### Scenario: Successful migration writes atomically
- **WHEN** all pending migrations succeed
- **THEN** the updated config SHALL be written using a temporary file and atomic rename where supported by the platform

### Requirement: Initial migrations are registered
The system SHALL register initial config migrations for the current schema. The initial migration set SHALL include a no-op baseline migration and a migration that updates legacy `claude-3-opus` model values to `claude-opus-4-0`.

#### Scenario: Legacy opus model is renamed
- **WHEN** a config file contains `model = "claude-3-opus"` and migrations run
- **THEN** the config file SHALL contain `model = "claude-opus-4-0"` after migration

#### Scenario: Non-legacy model is not changed
- **WHEN** a config file contains `model = "claude-sonnet-4-0"` and migrations run
- **THEN** the model value SHALL remain `claude-sonnet-4-0`

### Requirement: Startup runs config migrations before use
The CLI SHALL run pending migrations for the selected rust-claude config file before deserializing and applying runtime configuration.

#### Scenario: Migrated config affects runtime value
- **WHEN** the config file contains a legacy model value that a migration updates
- **THEN** the runtime configuration SHALL use the migrated model value

