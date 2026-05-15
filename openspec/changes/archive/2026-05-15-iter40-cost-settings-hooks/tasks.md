## 1. Cost Tracking Core

- [x] 1.1 Add `core::cost` module with `ModelPricing`, `get_pricing`, `calculate_cost`, `CostRecord`, `CostTracker`, and `BudgetStatus`
- [x] 1.2 Extend usage aggregation to include cache-read and cache-creation token counts where API usage provides them
- [x] 1.3 Add unit tests for Opus/Sonnet/Haiku pricing, unknown model fallback, cache token costing, tracker totals, and budget status

## 2. Settings Migration

- [x] 2.1 Add `core::migration` module with `Migration`, `MigrationRunner`, default migrations, and `_migration_version` handling
- [x] 2.2 Integrate migration execution into config loading before typed config deserialization
- [x] 2.3 Add unit tests for missing config files, pending migration order, migration failure behavior, and legacy Opus model rename

## 3. Hook Lifecycle and Once Execution

- [x] 3.1 Ensure hook config parsing supports `once` with backward-compatible default `false`
- [x] 3.2 Extend hook input payloads for `SessionStart` and `SessionEnd` with model, permission mode, duration, total cost, and message count
- [x] 3.3 Implement `HookRunner` once-state tracking so matching `once: true` hooks execute at most once per runner session
- [x] 3.4 Wire CLI session startup and shutdown paths to trigger `SessionStart` and `SessionEnd` hooks
- [x] 3.5 Add hook tests for once execution and lifecycle input payloads

## 4. Slash Command and Budget Integration

- [x] 4.1 Update `/cost` slash command output to use model-aware pricing and show cache token categories separately
- [x] 4.2 Add `maxBudgetUsd` config support and warn via output sink when tracked session cost exceeds budget
- [x] 4.3 Add tests for `/cost` detailed output and budget warning behavior

## 5. Verification

- [x] 5.1 Run `cargo check --workspace` and fix compilation errors
- [x] 5.2 Run targeted tests for cost, migration, hooks, and slash command changes
- [x] 5.3 Run `cargo test --workspace` and ensure the workspace passes
