## 1. Suggestion State And Candidate Sources

- [x] 1.1 Add TUI suggestion state to `crates/tui/src/app.rs`, including visibility, filtered candidates, selected index, and candidate kind metadata for commands versus skills
- [x] 1.2 Define a skill suggestion registry and unify command candidate derivation so slash suggestions can reuse the existing slash command registry data

## 2. Input Interaction Behavior

- [x] 2.1 Update input event handling so slash-prefixed buffers refresh suggestion state on insert, delete, paste, and cancel events
- [x] 2.2 Implement `Up` / `Down` navigation and `Enter` application semantics for visible suggestions while preserving existing history navigation and normal submit behavior when suggestions are inactive

## 3. Overlay Rendering

- [x] 3.1 Add a suggestion overlay renderer in `crates/tui/src/ui.rs` anchored above the input area with grouped `Commands` and `Skills` sections
- [x] 3.2 Render aligned name and description columns, selected-row highlighting, and bounded overlay height without disturbing existing input cursor positioning

## 4. Verification

- [x] 4.1 Add unit tests covering suggestion filtering, grouped rendering, arrow-key navigation, enter-to-apply behavior, and non-slash/history regressions
- [x] 4.2 Run `cargo test -p rust-claude-tui` and verify all suggestion-related tests pass
