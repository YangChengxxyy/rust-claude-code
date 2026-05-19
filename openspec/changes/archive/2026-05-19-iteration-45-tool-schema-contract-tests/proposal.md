## Why

Phase 5 iteration 45 needs stable contract coverage for built-in tool schemas so later compatibility work does not accidentally change model-facing inputs. This is especially important after iteration 44's file-tool field compatibility and iteration 39's deferred tool loading.

## What Changes

- Add contract tests that produce a stable summary for core tool schemas.
- Explicitly test that `FileRead`, `FileEdit`, and `FileWrite` accept both `path` and `file_path` inputs.
- Test that deferred tool discovery exposes full schema through `ToolSearch` while non-deferred tools remain excluded.
- Keep implementation scoped to the tools crate and avoid changing tool behavior beyond compatibility aliases.

## Capabilities

### New Capabilities
- `tool-schema-contract-tests`: Contract coverage for built-in tool schemas and deferred tool schema exposure.

### Modified Capabilities
- `tool-deferred-loading`: Add explicit contract coverage for deferred tool schema exposure via `ToolSearch`.

## Impact

- Affected code: `crates/tools/src/file_read.rs`, `crates/tools/src/file_edit.rs`, `crates/tools/src/file_write.rs`, and tools crate tests.
- Affected APIs: model-facing tool input schemas for file tools gain `file_path` compatibility while preserving existing `path` support.
- Dependencies: no new external dependencies.
