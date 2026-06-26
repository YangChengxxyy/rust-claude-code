## 1. Core Cost Tracking

- [x] 1.1 Add `crates/core/src/cost.rs` with `ModelPricing`, `ModelPricingFamily`, `get_pricing()`, and `calculate_cost()` covering input, output, cache read, and cache creation tokens.
- [x] 1.2 Add `CostRecord`, `ModelCostBreakdown`, `CostSummary`, `BudgetStatus`, and `CostTracker` with cumulative totals, per-model breakdowns, and 80% warning / 100% exceeded budget checks.
- [x] 1.3 Add unit tests for opus/sonnet/haiku/default pricing, cache-token cost calculation, per-model aggregation, and budget status thresholds.
- [x] 1.4 Export the cost module from `rust_claude_core` and update any workspace imports needed by `sdk` and `cli`.

## 2. Budget Configuration

- [x] 2.1 Add `max_budget_usd: Option<f64>` to `Config`, raw config parsing, config defaults, resolved field provenance, and config override structures.
- [x] 2.2 Support `RUST_CLAUDE_MAX_BUDGET_USD` as an environment override with validation for finite non-negative values.
- [x] 2.3 Add config unit tests for default budget, config-file budget, environment override budget, and invalid budget rejection.

## 3. Config Migration Infrastructure

- [x] 3.1 Add `crates/core/src/migration.rs` with a `Migration` trait and `MigrationRunner` that runs raw JSON migrations in ascending version order.
- [x] 3.2 Implement safe file handling for migrations: no-op when current version is latest, preserve unknown fields, write through a temporary file, and rename atomically on success.
- [x] 3.3 Register initial migrations: V1 no-op baseline and V2 legacy model rename from `claude-3-opus` to `claude-opus-4-0`.
- [x] 3.4 Add migration unit tests for missing `_migration_version`, up-to-date no-op behavior, unknown field preservation, legacy model rename, and failure leaving the original file unchanged.
- [x] 3.5 Run pending migrations from CLI startup before typed config loading for the selected rust-claude config path.

## 4. Agent Loop Integration

- [x] 4.1 Add a `CostTracker` to the session/application state path used by the agent loop, initialized with `config.max_budget_usd`.
- [x] 4.2 Update the tracker whenever API usage is received, using the model active for that request.
- [x] 4.3 Emit user-visible budget warnings through the existing output sink when budget status transitions to `Warning` or `Exceeded`.
- [x] 4.4 Add agent-loop tests that simulate usage updates, verify cumulative cost changes, and verify warning/exceeded notifications are not repeated for the same status.

## 5. /cost Command

- [x] 5.1 Update `/cost` slash-command handling to render cumulative input/output/cache tokens, total USD cost, pricing family, per-model breakdowns, and budget status.
- [x] 5.2 Ensure `/cost` works when no usage has been recorded and reports zero or unavailable usage without failing.
- [x] 5.3 Add CLI/TUI command tests for `/cost` with no usage, one model, multiple models, and configured budget status.

## 6. Verification

- [x] 6.1 Run `cargo test -p rust-claude-core` and fix any failures.
- [x] 6.2 Run `cargo test -p rust-claude-cli` and fix any failures.
- [x] 6.3 Run `cargo test -p rust-claude-sdk` if the package exists separately, otherwise run the package that owns `sdk/src/agent_loop.rs` tests.
- [x] 6.4 Run `cargo test --workspace` and document any remaining failures caused by external API-dependent tests.
