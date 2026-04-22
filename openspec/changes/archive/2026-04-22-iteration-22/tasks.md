## 1. Memory contract and models

- [x] 1.1 Add shared memory taxonomy types for `user`, `feedback`, `project`, and `reference`
- [x] 1.2 Add frontmatter-aware memory metadata models for memory name, description, type, and freshness-related fields
- [x] 1.3 Add prompt-building inputs for the memory contract, including save, no-save, recall, ignore, and verification guidance
- [x] 1.4 Add unit tests covering memory type parsing, missing/unknown type handling, and contract prompt assembly

## 2. Memory corpus and discovery

- [x] 2.1 Implement project-scoped memory store discovery using stable project identity and existing git-root/project-root logic
- [x] 2.2 Implement memory corpus scanning over per-memory Markdown files while treating `MEMORY.md` as a compact entrypoint rather than the primary payload
- [x] 2.3 Implement frontmatter metadata extraction and bounded `MEMORY.md` entrypoint loading for prompt-facing use
- [x] 2.4 Add tests covering memory store discovery, corpus scanning, frontmatter extraction, and entrypoint truncation behavior

## 3. Memory recall and maintenance

- [x] 3.1 Implement manifest generation from scanned memory metadata for recall candidate selection
- [x] 3.2 Implement bounded relevant-memory selection for a query, preserving freshness metadata for recalled memories
- [x] 3.3 Implement maintenance flows that write or update topic memory files first and update `MEMORY.md` afterward as bookkeeping
- [ ] 3.4 Implement memory correction, deduplication, and forgetting behavior consistent with the topic-file-first maintenance model
- [x] 3.5 Add tests covering recall selection, stale-memory framing, topic-file-first writes, index maintenance, and forgetting flows

## 4. Lightweight `/memory` command

- [x] 4.1 Add a lightweight `/memory` command that reports whether the current project has a memory store and where its entrypoint lives
- [x] 4.2 Extend `/memory` to display visible memory entries or lightweight memory metadata without implementing full editing workflow parity
- [ ] 4.3 Add CLI/TUI tests covering `/memory` output for projects with no memory store, an empty store, and a populated store

## 5. Compatibility follow-ups

- [x] 5.1 Add `--resume` / `-r` support for loading a specific saved session while preserving existing `--continue` behavior
- [x] 5.2 Extend shared content block and session persistence models to support image content blocks
- [x] 5.3 Implement `NotebookEditTool` for structured Jupyter cell edits with invalid-document and invalid-index safeguards
- [x] 5.4 Add tests covering explicit session resume, image content block round-tripping, and notebook editing behavior

## 6. End-to-end verification

- [x] 6.1 Verify system prompt construction includes the memory contract and compact memory entrypoint content in the expected order
- [x] 6.2 Verify memory subsystem behavior does not regress existing session, prompt, and slash-command flows across `core`, `cli`, `tools`, and `tui`
- [x] 6.3 Run `cargo test --workspace` and fix any regressions introduced by the memory subsystem and compatibility follow-ups
