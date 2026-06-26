## Context

Phase 4 already records API usage and emits session events, but usage is not converted into cost and config evolution still relies on additive `serde(default)` compatibility. The missing iteration 40 pieces affect multiple layers: `core` should own shared cost and migration types, `sdk` should update cost state from usage events, and `cli` should expose cost reporting and run migrations during startup.

The project already supports `Usage` data, session persistence, slash commands, hook events, and layered configuration resolution. The new behavior should fit those existing paths rather than introduce a separate accounting subsystem.

## Goals / Non-Goals

**Goals:**

- Provide deterministic model pricing lookup for opus, sonnet, haiku, and unknown model families.
- Calculate per-call cost from input, output, cache read, and cache creation tokens.
- Track cumulative session cost and per-turn cost records in memory.
- Support optional budget configuration via persisted config and environment/CLI override paths where appropriate.
- Surface `/cost` output through existing slash-command handling.
- Warn users when cumulative cost nears or exceeds the configured budget.
- Add a small versioned migration runner that updates config JSON before runtime values are used.

**Non-Goals:**

- Fetch live pricing from Anthropic or any remote service.
- Persist detailed per-turn cost history outside the existing session event model.
- Stop model calls automatically when budget is exceeded. This change warns; hard blocking can be a later policy feature.
- Build a general database migration framework. The scope is persisted JSON config migration only.
- Change existing authentication, provider routing, or API request behavior beyond cost accounting.

## Decisions

1. Put pricing and budget types in `core/src/cost.rs`.

   `core` already owns shared `Usage` and configuration types, and both `sdk` and `cli` need access to cost calculation. Keeping pricing pure and dependency-light makes it easy to unit test without network or tool execution.

   Alternative considered: put cost tracking in `cli` only. This would make `/cost` simple but prevent SDK-level event integration and future TUI reuse.

2. Use static model-family pricing rules.

   `get_pricing(model)` will match model names case-insensitively for `opus`, `sonnet`, and `haiku`, falling back to sonnet pricing for unknown names. This mirrors the iteration plan and avoids a runtime dependency on external pricing metadata.

   Alternative considered: exact model ID table. This is more precise but requires frequent updates and fails poorly for model aliases.

3. Make `CostTracker` an in-memory session object.

   The tracker should hold total cost, token totals, and per-turn records. It should be updated whenever usage is observed in the agent loop and queried by `/cost`. The existing session JSONL stream can continue recording usage events; cost can be recomputed or summarized later if needed.

   Alternative considered: persist every cost record as a new session event immediately. That adds migration and replay complexity before the UI needs historical cost reconstruction.

4. Warn at budget thresholds but do not block calls.

   `BudgetStatus` should return `Ok`, `Warning`, or `Exceeded`. The initial warning threshold should be 80% of `max_budget_usd`, with `Exceeded` at or above the limit. The agent loop should emit a user-visible warning when status changes into warning or exceeded to avoid repeated noise.

   Alternative considered: stop sending API requests after exceeding budget. That changes agent behavior and needs a confirmation/recovery UI that is outside this change.

5. Put migration infrastructure in `core/src/migration.rs` and invoke it from CLI config startup.

   Migrations operate on raw `serde_json::Value` before deserializing into typed `Config`. The runner reads `_migration_version`, runs pending migrations in ascending order, writes the updated file atomically, and then normal config loading proceeds.

   Alternative considered: deserialize first and migrate typed `Config`. Raw JSON migration can rename/remove fields without losing unknown data and is safer for future config shape changes.

6. Keep initial migrations minimal.

   The first migration is a V1 baseline/no-op that records the current schema version. A V2 example migration can rename legacy `claude-3-opus` model values to `claude-opus-4-0` if present, matching the phase plan while staying harmless for other configs.

## Risks / Trade-offs

- Pricing may become stale -> Keep pricing rules centralized in `core::cost` with unit tests, making updates cheap.
- Budget warning spam could degrade UX -> Track the last emitted budget status and only notify on status transitions.
- Raw JSON migration may corrupt config if write is interrupted -> Write to a temporary file and rename atomically.
- Unknown model aliases may use inaccurate default pricing -> Use sonnet default and show the model name in `/cost` so users can identify assumptions.
- Config migration errors could block startup -> Return a clear error including migration version and file path; do not silently continue with partially migrated config.

## Migration Plan

- Add migration runner and baseline migrations without changing existing config semantics.
- On startup, run migrations for the selected config path before typed config loading.
- If migration succeeds, `_migration_version` is written to config JSON.
- If migration fails, startup reports an actionable error and leaves the original config file intact.
- Rollback is removing the migration invocation; migrated config remains valid because added fields are either ignored or typed with defaults.
