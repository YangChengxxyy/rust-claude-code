## Context

The TypeScript reference implementation treats memory as a subsystem, not just as a persistent note store. Its design combines several layers:

- a typed memory taxonomy (`user`, `feedback`, `project`, `reference`)
- explicit behavioral rules for what should and should not be saved
- a compact `MEMORY.md` entrypoint that is always prompt-visible
- per-memory Markdown files with frontmatter metadata as the actual memory corpus
- a relevance-based recall step that selects a small number of memory files for a given query
- freshness and trust safeguards so stale memories are treated as historical context rather than current truth
- a user-facing `/memory` management surface

The current Rust iteration-22 proposal initially over-centered `MEMORY.md` as the primary index and treated `/memory` as a simple listing command. After comparing against the reference source, the design goal is now clearer: align the memory subsystem internals first, while intentionally keeping `/memory` lightweight in this iteration.

The repository already has several prerequisites that make this feasible without introducing a separate architectural track:

- project-aware configuration and git-root discovery
- system prompt composition infrastructure
- session persistence
- slash command handling
- existing file-oriented tools and shared content block/message models

The main design challenge is to bring over the real shape of the memory system without pulling in the entire TypeScript UX surface at once.

## Goals / Non-Goals

**Goals:**

- Define a typed memory subsystem that matches the reference design more closely than the current proposal.
- Treat per-memory Markdown files as the primary memory corpus.
- Treat `MEMORY.md` as a concise prompt-facing entrypoint and mechanical index rather than the primary memory payload.
- Encode memory behavior rules into prompt construction so the model has explicit guidance for saving, ignoring, recalling, and verifying memory.
- Add a recall pipeline that can select relevant memory files for a query from scanned metadata.
- Add a maintenance model where topic files are the real unit of memory mutation and `MEMORY.md` updates are bookkeeping.
- Provide a lightweight `/memory` command for visibility into the current memory store and visible memory entries.
- Add explicit session resume selection, image content block support, and notebook editing as secondary compatibility items.

**Non-Goals:**

- Replicate the full TypeScript `/memory` interactive editing workflow in this iteration.
- Add full team-memory parity, synchronization, or organization-level memory behavior unless explicitly scoped in later.
- Recreate every feature-flagged branch of the TypeScript memory system.
- Build a separate memory database, background sync service, or hidden storage layer outside the filesystem.
- Implement rich image rendering in the TUI.
- Implement full notebook execution behavior; notebook work is limited to structured editing.

## Decisions

### 1. Make per-memory files the primary corpus

The Rust design will treat frontmatter-backed memory files as the actual durable memory corpus. These files are the units that hold memory content, are scanned for metadata, are selected for recall, and are updated or removed when memory changes.

This follows the reference implementation more closely than an index-first design. In the TypeScript source, memory scanning excludes `MEMORY.md` and reads metadata from the topic files themselves. That indicates the real memory payload lives in the per-memory files, not in the entrypoint index.

Why this approach:

- it matches the observed `memoryScan.ts` behavior
- it creates a cleaner separation between memory content and memory navigation
- it makes later recall selection and freshness handling more natural

Alternative considered: making `MEMORY.md` the canonical store and treating per-topic files as optional expansions. Rejected because it inverts the reference design and would make recall and maintenance logic less accurate.

### 2. Treat `MEMORY.md` as a prompt-facing entrypoint and mechanical index

`MEMORY.md` will be modeled as a compact entrypoint file that is loaded into prompt context and maintained as an index over the topic files. It is not the primary place where memory content lives.

This aligns with multiple signals in the reference implementation:

- extraction prompts describe `MEMORY.md` as "an index, not a memory"
- actual recall selection excludes `MEMORY.md`
- `MEMORY.md` is specially truncated for prompt use
- updates to `MEMORY.md` are described as mechanical bookkeeping

Why this approach:

- it preserves prompt budget by keeping the index concise
- it avoids duplicating memory content in both the topic file and the entrypoint
- it reflects how the reference system distinguishes memory payload from memory navigation

Alternative considered: parsing `MEMORY.md` first and treating it as the authoritative inventory of all memory. Rejected because the reference implementation is entry-file-first, with `MEMORY.md` serving as entrypoint rather than sole truth source.

### 3. Make the memory contract a first-class part of prompt construction

The memory system is not only a storage mechanism; it is also a behavioral contract for the model. The Rust design should explicitly encode:

- the memory taxonomy
- what kinds of information are worth saving
- what kinds of information should not be saved
- when memory should be accessed
- what "ignore memory" means
- how recalled memories should be verified before being treated as present truth

This is a direct consequence of the TypeScript reference design, where these rules are materialized as large prompt sections rather than implicit comments in code.

Why this approach:

- it reduces the chance that the implementation becomes a passive storage feature without behavioral discipline
- it preserves the strongest part of the reference design: prompt-level governance of memory behavior
- it supports future auto-extraction and update flows without inventing policy ad hoc

Alternative considered: adding only storage mechanics now and leaving prompt rules for later. Rejected because storage without contract would produce a superficially complete but behaviorally incorrect memory system.

### 4. Add a manifest-based recall selection step

Memory recall will not rely only on the always-loaded `MEMORY.md` entrypoint. Instead, the system will scan memory headers, build a manifest, and select a small set of relevant memory files for a given query.

This follows the structure of the TypeScript `findRelevantMemories` flow:

- scan frontmatter metadata from topic files
- format a compact manifest
- use a side model to select the most relevant files
- carry freshness metadata forward

Why this approach:

- it avoids flooding the main model with the entire memory corpus
- it preserves the distinction between a compact global index and per-query relevant context
- it creates a natural place to attach freshness and trust cues

Alternative considered: always load only `MEMORY.md`, or always load all memory files. Rejected because the first loses useful detail and the second is too expensive and too noisy.

### 5. Preserve freshness metadata and treat memory as historical context

Memory entries can become stale. The design should preserve timestamps or equivalent freshness metadata so older memories can be surfaced with caution rather than asserted as live truth.

The reference implementation explicitly models memory age and uses freshness text to warn that file paths, functions, flags, and behavior claims may have changed since a memory was written.

Why this approach:

- it reduces the risk of stale or authoritative-sounding hallucinations
- it aligns with the reference implementation's recall safety posture
- it gives the Rust implementation a principled way to distinguish memory from live repo state

Alternative considered: loading memory without freshness semantics and relying only on general model caution. Rejected because the reference implementation treats freshness as explicit, not implicit.

### 6. Separate memory maintenance from memory presentation

When the system creates or updates memory, the memory file itself is the primary mutation target. `MEMORY.md` should be updated afterward as index maintenance. Forgetting or correcting memory should similarly target the topic files first.

This mirrors the extraction prompt structure in the reference implementation:

- write the memory file
- then add or update the pointer in `MEMORY.md`
- avoid duplicates
- update or remove stale memories

Why this approach:

- it keeps the memory corpus authoritative
- it prevents `MEMORY.md` from becoming an overloaded knowledge file
- it maps cleanly to future automated extraction behavior

Alternative considered: treating `MEMORY.md` edits as the main mutation path and deriving topic files from it. Rejected because it reverses the reference workflow.

### 7. Keep `/memory` lightweight in this iteration

The TypeScript `/memory` command is a management surface with file selection, creation, editing, folder-opening, and feature toggles. This iteration will not try to reproduce that full workflow.

Instead, `/memory` in Rust should be a lightweight inspection surface:

- show whether the current project has a memory store
- show key entrypoint paths
- show visible memory entries or summaries
- help the user understand what memory exists and where it lives

Why this approach:

- it keeps iteration-22 centered on the memory subsystem rather than the UI parity layer
- it avoids expanding the change into a broader file manager/editor workflow
- it still gives users enough visibility to debug and inspect the system

Alternative considered: building full TypeScript `/memory` parity immediately. Rejected for scope reasons; it is better suited to a follow-up change once the subsystem core is in place.

### 8. Keep room for future memory scopes without requiring them now

The reference implementation distinguishes individual-only and combined private/team memory modes and also has adjacent concepts like agent memory. This iteration should not block those future directions, but it does not need to fully implement them now.

The Rust design should therefore avoid assumptions that there will only ever be one memory store or one memory entrypoint forever. The initial implementation can focus on the primary project-scoped memory store while leaving the model extensible.

Why this approach:

- it prevents redesign later when team or agent memory arrives
- it respects what the reference implementation already signals
- it avoids taking on scope that is not necessary for iteration-22

Alternative considered: adding full private/team/agent memory parity now. Rejected as too large for the current change.

## Risks / Trade-offs

- [Scope drift from UX parity] -> Keep `/memory` explicitly lightweight and document full workflow parity as a follow-up concern.
- [Over-centering `MEMORY.md`] -> Make per-memory files the primary corpus in the design and keep `MEMORY.md` bookkeeping-focused.
- [Prompt budget pressure] -> Keep `MEMORY.md` compact, rely on recall selection, and avoid full-corpus prompt injection.
- [Behavior drift without strong rules] -> Encode the memory contract directly into prompt construction rather than leaving it as implementation folklore.
- [Stale memory misuse] -> Preserve freshness metadata and include verification-oriented recall guidance.
- [Future architecture lock-in] -> Keep the design compatible with multiple memory scopes without implementing them all now.

## Migration Plan

This change is additive and can be rolled out incrementally:

1. Add shared memory taxonomy and metadata models.
2. Add memory store discovery and frontmatter-backed corpus scanning.
3. Add prompt construction support for the memory contract and entrypoint injection.
4. Add recall selection and freshness-aware memory surfacing.
5. Add maintenance logic for topic-file-first memory updates and `MEMORY.md` bookkeeping.
6. Add the lightweight `/memory` surface.
7. Add secondary compatibility items (`--resume`, image content blocks, notebook editing).
8. Verify backward compatibility for session persistence and prompt assembly.

Rollback remains straightforward because the new layers are additive: memory prompt injection can be disabled, the command surface can be hidden, and compatibility items can be isolated without reverting the broader application model.

## Open Questions

- Should the first Rust version of the memory subsystem include auto-extraction hooks now, or should iteration-22 only establish the core primitives that later automation can build on?
- How much of the memory contract should be injected as static system prompt text versus dynamically attached only when memory is present?
- Should lightweight `/memory` show only the auto-memory store, or also expose broader instruction-style memory entrypoints the way the TypeScript UX does?
- Should freshness cues be attached only to dynamically recalled topic files, or also influence how the always-loaded `MEMORY.md` entrypoint is framed?
- How much of the future private/team/agent memory distinction should be reflected in the initial Rust type model?
