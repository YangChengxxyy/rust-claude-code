## Context

Iteration 45 follows the phase 5 protocol compatibility work. The current tools crate already has `ToolInfo` schemas, `ToolRegistry` ordering, `ToolSearchTool`, and deferred tool support. File tools currently use `path` internally, while original Claude Code compatibility expects `file_path` to be accepted as an alias.

## Goals / Non-Goals

**Goals:**
- Protect built-in tool schema shapes with stable, readable contract tests.
- Verify file tools accept both Rust-native `path` and Claude Code-compatible `file_path`.
- Verify deferred tools can expose full schemas through `ToolSearch` and remain searchable through registry behavior.

**Non-Goals:**
- Do not introduce an external snapshot testing dependency.
- Do not require byte-for-byte equality with original TypeScript JSON schemas.
- Do not rename existing Rust tool names or replace internal `PathBuf` usage.
- Do not add new runtime tools.

## Decisions

1. Use inline normalized schema summaries instead of filesystem snapshots.
   - Rationale: the tools crate can keep schema contracts close to existing unit tests without introducing a new dependency or snapshot file update workflow.
   - Alternative considered: adding `insta` snapshot tests. Rejected because iteration 45 explicitly only needs a stable contract and no new dependency.

2. Keep `path` as the canonical internal field and accept `file_path` at deserialization boundaries.
   - Rationale: existing Rust code and TUI paths already use `path`, while compatibility only requires model/tool input acceptance.
   - Alternative considered: renaming schemas to only `file_path`. Rejected because it would be a broader breaking change.

3. Test deferred schema exposure through both registry search and `ToolSearchTool` execution.
   - Rationale: registry search proves the low-level contract; executing `ToolSearchTool` proves the model-facing output contains full schema fields.

## Risks / Trade-offs

- Schema summaries can become noisy if every description change fails tests → limit summaries to tool name, required fields, property names, and deferred flag.
- Supporting both `path` and `file_path` means schemas are slightly more permissive → require tests that both aliases work and that missing both remains invalid.
- Deferred tool behavior depends on registered test tools → keep test registry small and deterministic.

## Migration Plan

No migration is required. Existing callers using `path` continue to work; callers using `file_path` become accepted.
