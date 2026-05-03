## ADDED Requirements

### Requirement: File read recording
The `FileReadTool` SHALL record every successful file read into the `FileStateCache`. The record SHALL include the canonical file path, content hash, file mtime, offset, limit, and `is_partial_view` flag.

#### Scenario: Full file read recorded
- **WHEN** `FileReadTool` reads `/Users/cc/projects/src/main.rs` with no offset/limit
- **THEN** the cache SHALL store an entry with the file's content hash, current mtime, `offset: None`, `limit: None`, and `is_partial_view: false`

#### Scenario: Partial file read recorded
- **WHEN** `FileReadTool` reads a file with `offset: 10, limit: 50`
- **THEN** the cache SHALL store an entry with `offset: Some(10)`, `limit: Some(50)`, and `is_partial_view: false`

#### Scenario: System-injected content recorded as partial view
- **WHEN** the system auto-injects file content (e.g., CLAUDE.md in system prompt) without a user-initiated FileRead
- **THEN** the cache SHALL store an entry with `is_partial_view: true`

### Requirement: Staleness detection before file writes
`FileEditTool` and `FileWriteTool` SHALL check `FileStateCache::is_stale(path)` before performing any write operation. If the file is stale, the tool SHALL return an error instructing the model to re-read the file first.

#### Scenario: File not modified since last read
- **WHEN** `FileEditTool` attempts to edit `/src/main.rs`
- **AND** the file's mtime has not changed since the cached read
- **THEN** the edit SHALL proceed normally

#### Scenario: File modified externally since last read
- **WHEN** `FileEditTool` attempts to edit `/src/main.rs`
- **AND** the file's mtime has changed since the cached read
- **THEN** the tool SHALL return a `ToolResult::error` with message "File has been modified since last read. Please re-read the file before editing."
- **AND** the edit SHALL NOT be performed

#### Scenario: File not previously read
- **WHEN** `FileWriteTool` attempts to write to a path that has no cache entry
- **THEN** the write SHALL proceed normally (no staleness check for unknown files)

#### Scenario: FileWriteTool with stale file
- **WHEN** `FileWriteTool` attempts to write to a path whose cached mtime differs from current mtime
- **THEN** the tool SHALL return a `ToolResult::error` with message "File has been modified since last read. Please re-read the file before writing."

### Requirement: Partial view edit rejection
`FileEditTool` and `FileWriteTool` SHALL reject operations on files whose cache entry has `is_partial_view: true`, requiring a full `FileRead` first.

#### Scenario: Attempt to edit a partial-view file
- **WHEN** `FileEditTool` attempts to edit a file cached with `is_partial_view: true`
- **THEN** the tool SHALL return a `ToolResult::error` with message "File was read as partial view (system-injected). Please read the file with FileRead before editing."
- **AND** the edit SHALL NOT be performed

#### Scenario: Full read clears partial view flag
- **WHEN** a file was previously cached with `is_partial_view: true`
- **AND** `FileReadTool` subsequently reads the same file (full or partial user-initiated read)
- **THEN** the cache entry SHALL be updated with `is_partial_view: false`
- **AND** subsequent edits SHALL be allowed (subject to staleness check)

### Requirement: LRU cache bounds
The `FileStateCache` SHALL use LRU eviction with a maximum of 100 entries. When the cache is full and a new entry is added, the least recently used entry SHALL be evicted.

#### Scenario: Cache at capacity
- **WHEN** the cache contains 100 entries
- **AND** a new file is read
- **THEN** the least recently used entry SHALL be evicted
- **AND** the new entry SHALL be stored

#### Scenario: Accessing an entry refreshes its position
- **WHEN** `is_stale(path)` is called for a cached file
- **THEN** the entry's LRU position SHALL be refreshed (it becomes most recently used)

### Requirement: Cache update after successful write
After a successful `FileEditTool` or `FileWriteTool` operation, the cache entry for that file SHALL be updated with the new content hash and mtime.

#### Scenario: Cache updated after edit
- **WHEN** `FileEditTool` successfully edits `/src/main.rs`
- **THEN** the cache entry for `/src/main.rs` SHALL be updated with the new file's mtime and content hash
- **AND** `is_partial_view` SHALL be set to `false`

#### Scenario: Cache updated after write
- **WHEN** `FileWriteTool` successfully writes to `/src/config.rs`
- **THEN** the cache entry SHALL be created or updated with the new file's mtime and content hash

### Requirement: Mtime edge case handling
When the file mtime is identical to the cached mtime but within 2 seconds of the read time, the system SHALL re-read the file and compare content hashes to detect modifications that occurred within the same filesystem timestamp granularity.

#### Scenario: Modification within same mtime second
- **WHEN** a file was read at time T and its mtime is T (same second)
- **AND** the file was externally modified at time T+0.5s (same mtime due to 1-second granularity)
- **AND** `is_stale()` is called within 2 seconds of the original read
- **THEN** the system SHALL re-read the file and compare content hashes
- **AND** if the hash differs, the file SHALL be reported as stale
