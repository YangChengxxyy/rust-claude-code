## Why

Iteration 22 introduced the memory subsystem, session resume, image content blocks, and notebook editing. Two tasks remain unchecked: memory deduplication/correction/forgetting behavior (task 3.4) and CLI/TUI tests for `/memory` output states (task 4.3). Completing these closes the iteration and satisfies the memory-maintenance spec requirements that are currently unmet.

## What Changes

- Add a `find_duplicate_memory` function that scans existing memory entries by path or topic name to detect when an incoming write would create a duplicate, so `write_memory_entry` can update rather than create.
- Add a `correct_memory_entry` function that updates the body and/or frontmatter of an existing memory file in-place, then rebuilds the `MEMORY.md` index.
- Extend the existing `remove_memory_entry` to support batch removal of stale entries based on a caller-supplied predicate, enabling future automated cleanup.
- Add integration-style tests in `crates/cli` that exercise `format_memory_status` against temp directories representing no-store, empty-store, and populated-store states.
- Add TUI-level tests in `crates/tui` that verify `/memory` dispatches the correct `UserCommand` variants and that output formatting handles all three store states.

## Capabilities

### New Capabilities
- `memory-dedup-correction`: Deduplication detection and in-place memory correction logic that prevents duplicate memories and supports targeted updates to existing topic files.

### Modified Capabilities
- `memory-maintenance`: Add the deduplication-aware write path and correction flow required by the existing spec but not yet implemented.
- `memory-command-lite`: Add test coverage for `/memory` output across no-store, empty-store, and populated-store states.

## Impact

- `crates/core/src/memory.rs`: New public functions for duplicate detection and in-place correction; minor extension to removal API.
- `crates/cli/src/main.rs`: Update `remember_memory` to use dedup-aware write path; add integration tests for `format_memory_status`.
- `crates/tui/src/app.rs`: Add tests for `/memory` output formatting across store states.
- No API changes, no new dependencies, no breaking changes.
