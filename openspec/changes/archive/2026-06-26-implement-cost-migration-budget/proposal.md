## Why

Phase 4 iteration 40 still lacks the cost accounting, budget enforcement, and configuration migration pieces needed to make long-running sessions operationally predictable. Without these, users cannot see accurate per-model spend, set a budget guardrail, or safely evolve persisted configuration beyond `serde(default)` compatibility.

## What Changes

- Add model-aware cost calculation that accounts for input, output, cache read, and cache creation tokens.
- Add a session-level `CostTracker` with per-turn records, total cost accumulation, warning/exceeded budget status, and configurable `max_budget_usd`.
- Add `/cost` output that reports total cost and a breakdown by model and token category.
- Add configuration migration infrastructure with versioned migrations stored in config as `_migration_version`.
- Run pending migrations during configuration loading before runtime settings are used.
- Emit budget warnings through the existing output path when cumulative cost approaches or exceeds the configured budget.

## Capabilities

### New Capabilities
- `cost-tracking`: Model-aware cost calculation, per-session cost accumulation, `/cost` reporting, and budget status checks.
- `config-migration`: Versioned configuration migrations that upgrade persisted config files safely at startup.

### Modified Capabilities
- `settings-merge`: Runtime configuration now includes a budget field and executes migrations before applying settings.
- `slash-command-extensions`: Add `/cost` behavior for inspecting current session spend.

## Impact

- Affected crates: `core` for cost and migration types, `cli` for config loading and `/cost`, and `sdk` for usage-to-cost integration in the agent loop.
- Affected persisted data: `config.json` gains `_migration_version` and optional `max_budget_usd`.
- No breaking CLI changes are expected; new behavior is additive.
- No new external service dependencies are required.
