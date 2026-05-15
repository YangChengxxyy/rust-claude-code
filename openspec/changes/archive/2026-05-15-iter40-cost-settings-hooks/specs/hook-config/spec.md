## ADDED Requirements

### Requirement: Hook once default compatibility
Hook configuration parsing SHALL treat a missing `once` field as `false` and SHALL preserve existing hook configurations that do not specify `once`.

#### Scenario: Existing hook config without once
- **WHEN** settings.json contains a command hook with no `once` field
- **THEN** the parsed hook config SHALL set `once` to false
