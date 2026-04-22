## Why

The Rust implementation still lacks the core memory subsystem shape used by the TypeScript reference implementation. What is missing is not only a place to persist cross-session notes, but a governed memory layer: typed memory files, a compact prompt-facing `MEMORY.md` entrypoint, explicit rules for what memory should and should not capture, relevance-based recall, and maintenance behavior for updating or removing stale memories over time.

This change focuses on aligning that memory subsystem first. It intentionally keeps the `/memory` command lightweight in this iteration, providing a minimal inspection surface rather than trying to reproduce the full TypeScript memory management workflow in one pass. The iteration also includes a small set of compatibility follow-ups that are adjacent but secondary to the memory subsystem itself.

## What Changes

- Add a typed memory subsystem backed by per-memory Markdown files with frontmatter metadata.
- Treat `MEMORY.md` as a concise prompt-facing entrypoint and mechanical index, not as the primary memory payload.
- Add memory behavior rules to the system prompt, including what to save, what not to save, when to access memory, when to ignore memory, and how to verify recalled facts before acting on them.
- Add a memory recall pipeline that scans memory file headers, selects relevant memory files for a query, and preserves freshness metadata so old memories are treated as historical context rather than live truth.
- Add a memory maintenance workflow that writes or updates topic memory files first and updates `MEMORY.md` afterward as bookkeeping.
- Add a lightweight `/memory` command surface for inspecting the current memory store and visible memory entries without implementing the full TypeScript editing and folder-management workflow in this iteration.
- Add support for resuming a specific saved session via `--resume` / `-r`.
- Add foundational image content block support so requests and persisted sessions can represent image inputs alongside existing text, tool, and thinking blocks.
- Add a basic notebook editing tool for structured Jupyter cell edits.

## Capabilities

### New Capabilities
- `memory-contract`: Define the memory taxonomy and the behavioral rules for saving, recalling, ignoring, and trusting memory.
- `memory-corpus`: Discover and manage the project-scoped memory store made of frontmatter-backed memory files plus a concise `MEMORY.md` entrypoint.
- `memory-recall-selection`: Select relevant memory files for a query from scanned memory metadata instead of relying only on the always-loaded index.
- `memory-maintenance`: Support memory creation, update, deduplication, forgetting, and mechanical `MEMORY.md` updates as part of a maintained memory workflow.
- `memory-command-lite`: Provide a lightweight `/memory` command for inspecting memory entrypoints and visible memory content without full workflow parity.
- `session-resume`: Resume an explicitly selected saved session through new CLI flags.
- `image-content-blocks`: Represent, serialize, and persist image content blocks in the shared message model.
- `notebook-edit-tool`: Edit Jupyter notebook cells through a structured tool workflow.

### Modified Capabilities
- `slash-command-extensions`: Extend the built-in slash command surface to support the lightweight `/memory` command.
- `claude-md-injection`: Extend system prompt assembly so it can inject the memory contract and memory entrypoint content in a TS-aligned way.

## Impact

- `crates/core`: add memory taxonomy and metadata types, frontmatter-aware memory models, freshness metadata handling, and image content block support.
- `crates/cli`: add memory store discovery, memory-aware system prompt construction, memory recall orchestration, lightweight `/memory` command handling, and explicit session resume selection.
- `crates/tools`: add notebook editing support and any supporting integration needed by the agent loop.
- `crates/tui`: add lightweight memory inspection presentation and safe rendering for image-aware messages.
- Session persistence and prompt construction: update persistence and prompt flows to preserve new image content blocks and memory-related context behavior.
- Tests and OpenSpec specs: add coverage for memory contract behavior, memory corpus discovery, recall selection, maintenance behavior, the lightweight `/memory` surface, and the compatibility follow-ups.
