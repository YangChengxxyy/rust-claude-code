## Why

The current system only has proactive compaction (threshold check before API requests), but when the API actually returns `prompt-too-long` / `invalid_request_error` (HTTP 400), the system errors out with no recovery. Long conversations can crash mid-session. Additionally, when the model is overloaded (HTTP 529), the system can only retry — there is no ability to fall back to a backup model, leaving users stuck during high-demand periods.

## What Changes

- Add reactive compaction: catch `prompt-too-long` errors from the API and automatically compact + retry, with a micro-compaction fallback that strips old tool results
- Add model fallback: after consecutive 529 (overloaded) errors exceed a threshold, auto-switch to a configured fallback model and notify the user
- Add new `ApiError` variants (`PromptTooLong`, `Overloaded`) with proper HTTP response parsing
- Add `fallback_model` configuration option (config file, settings.json, environment variable)
- Add `micro_compact()` method to the compaction service for lightweight context reduction

## Capabilities

### New Capabilities
- `reactive-compaction`: Catch API prompt-too-long errors, trigger compaction reactively, and implement micro-compaction (stripping old tool results) as a second-level fallback
- `model-fallback`: Track consecutive 529 overloaded errors and auto-switch to a configured fallback model, with user notification and counter reset on success

### Modified Capabilities
- `compaction-engine`: Adding micro-compaction method that replaces old ToolResult content with summary placeholders while preserving recent turns

## Impact

- **`sdk` crate (`agent_loop.rs`)**: New error-handling branches in the main agent loop for prompt-too-long and 529 errors, new `RetryState` struct
- **`sdk` crate (`compaction.rs`)**: New `micro_compact()` method
- **`core` crate (`config.rs`)**: New `fallback_model` field in Config
- **`api` crate (`error.rs`)**: New `PromptTooLong` and `Overloaded` error variants, updated HTTP response parsing
- **Settings integration**: `settings.json` `fallbackModel` field, `RUST_CLAUDE_FALLBACK_MODEL` env var
