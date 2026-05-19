## 1. File tool compatibility

- [x] 1.1 Add a shared deserialization pattern so `FileRead`, `FileEdit`, and `FileWrite` accept both `path` and `file_path` while preserving canonical internal paths.
- [x] 1.2 Add explicit file-tool tests for `path`, `file_path`, and missing-path validation.

## 2. Schema contract coverage

- [x] 2.1 Add built-in tool schema summary tests covering stable names, deferred flags, required fields, and top-level properties.
- [x] 2.2 Add deferred schema exposure tests for `ToolSearch` output and non-deferred exclusion.

## 3. Verification and documentation

- [x] 3.1 Run `cargo test -p rust-claude-tools schema` and `cargo test -p rust-claude-tools registry`.
- [x] 3.2 Update `doc/phase5-iteration-plan.md` to mark iteration 45 complete after verification.
