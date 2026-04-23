## Context

Iteration 22 introduced the memory subsystem with typed memory files, corpus scanning, recall selection, maintenance (write/remove), and a lightweight `/memory` command. Two tasks remain:

- **Task 3.4**: Memory correction, deduplication, and forgetting behavior. The current `write_memory_entry` blindly overwrites if the path exists but has no semantic duplicate detection. `remove_memory_entry` works for single files but there is no facility for batch cleanup or topic-aware dedup.
- **Task 4.3**: CLI/TUI tests for `/memory` output. The `/memory` command is fully wired (show, remember, forget), but there are no tests exercising the formatted output for different store states (no store, empty store, populated store).

The existing memory module (`crates/core/src/memory.rs`) has 10 public functions, a complete frontmatter parser, index rebuilder, and scan pipeline. The additions are small and surgical.

## Goals / Non-Goals

**Goals:**

- Add a `find_duplicate_memory` function that checks whether an incoming write would create a near-duplicate of an existing entry, enabling callers to update instead of create.
- Add a `correct_memory_entry` function that updates the frontmatter and/or body of an existing memory file in-place and rebuilds the index.
- Add test coverage for `format_memory_status` across no-store, empty-store, and populated-store scenarios.
- Add TUI-level tests verifying `/memory` output message formatting for each store state.

**Non-Goals:**

- Automated background deduplication or pruning. Dedup detection is a helper for callers; it does not run automatically.
- Semantic similarity matching. Duplicate detection uses path matching and exact name matching, not NLP or embedding similarity.
- Full interactive `/memory` editing workflow parity with TypeScript.
- Changes to the API crate or streaming infrastructure.

## Decisions

### 1. Path-and-name based duplicate detection

`find_duplicate_memory` will scan the existing `ScannedMemoryStore` entries and return a match if:
- The incoming `relative_path` matches an existing entry's `relative_path` (exact path duplicate), OR
- The incoming `frontmatter.name` matches an existing entry's `frontmatter.name` (case-insensitive, topic duplicate)

This is deliberately simple. The reference TypeScript implementation relies on the model itself to detect semantic duplicates via prompt instructions ("avoid duplicates"). The Rust helper provides a mechanical safety net, not a semantic one.

Alternative considered: fuzzy/embedding-based matching. Rejected because it would require an external dependency or side-model call, which is disproportionate for this wrap-up.

### 2. `correct_memory_entry` as thin wrapper

`correct_memory_entry` takes a `MemoryStore`, a `relative_path`, and a new `MemoryWriteRequest`. It verifies the target file exists, writes the new content, and rebuilds the index. It returns an error if the file does not exist (distinguishing correction from creation).

This keeps the API surface minimal. The caller (CLI `remember_memory`) can first call `find_duplicate_memory`, decide whether to update or create, and then call `correct_memory_entry` or `write_memory_entry` accordingly.

Alternative considered: a single `upsert_memory_entry` function. Rejected because the decision about whether to update vs. create should be explicit, not hidden inside an upsert.

### 3. Test strategy for `/memory` output

The CLI tests will call `format_memory_status` directly against temp directories:
- **No store**: temp dir with no `.claude/` project structure -> output says "no memory store"
- **Empty store**: memory dir exists but contains no `.md` files -> output says "0 entries"
- **Populated store**: memory dir with 2-3 entries -> output lists entries with type/description

The TUI tests will use the existing `AppTestHarness` pattern to verify that `/memory` dispatches `UserCommand::ShowMemory` and that the formatted response appears as a `ChatMessage`.

## Risks / Trade-offs

- [Name-based dedup may miss semantic duplicates] -> Acceptable because the model's prompt-level contract already covers semantic dedup. The function is a mechanical fallback.
- [Tests use temp dirs, not real project structures] -> Sufficient for unit-level coverage. Real integration is already verified by the existing end-to-end checks.
- [No batch removal API added] -> The existing `remove_memory_entry` is adequate for single-file removal. Batch cleanup is a future concern when automated extraction is implemented.
