## 1. Memory deduplication and correction in core

- [x] 1.1 Add `find_duplicate_memory` function to `crates/core/src/memory.rs` that scans a `ScannedMemoryStore` for entries matching by `relative_path` (exact) or `frontmatter.name` (case-insensitive), returning `Option<&MemoryEntry>`
- [x] 1.2 Add `correct_memory_entry` function to `crates/core/src/memory.rs` that overwrites an existing memory file's frontmatter and body, then rebuilds the index; returns error if the target file does not exist
- [x] 1.3 Add unit tests for `find_duplicate_memory`: exact path match, name match (case-insensitive), and no-match cases
- [x] 1.4 Add unit tests for `correct_memory_entry`: successful correction with index rebuild, and error on nonexistent target

## 2. Dedup-aware CLI memory write path

- [x] 2.1 Update `remember_memory` in `crates/cli/src/main.rs` to call `find_duplicate_memory` before writing; if a duplicate is found, call `correct_memory_entry` instead of `write_memory_entry`, and adjust the user-facing message to say "updated" vs "created"
- [x] 2.2 Add a unit test verifying that `remember_memory` updates an existing entry when a duplicate path is detected

## 3. `/memory` output tests

- [x] 3.1 Add CLI-level tests in `crates/cli` that call `format_memory_status` against temp directories for: no memory store, empty memory store (dir exists but no entries), and populated memory store (2+ entries with frontmatter)
- [x] 3.2 Add TUI-level tests in `crates/tui/src/app.rs` verifying that `/memory` dispatches `UserCommand::ShowMemory` and that the response path correctly formats output for each store state

## 4. End-to-end verification

- [x] 4.1 Run `cargo test --workspace` and fix any regressions introduced by the new dedup/correction functions and new tests
- [x] 4.2 Verify the existing 443 tests continue to pass alongside the new additions
