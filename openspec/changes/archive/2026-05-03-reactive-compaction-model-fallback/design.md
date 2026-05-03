## Context

The agent loop (`agent_loop.rs`) currently performs proactive compaction before each API call by checking if the estimated token count exceeds 80% of the context window. However, the token estimation uses a chars/4 heuristic that can be inaccurate. When the API returns HTTP 400 with `invalid_request_error` (prompt too long), or HTTP 529 (overloaded), these errors propagate as fatal `QueryLoopError::Api` and terminate the entire `run()` call with no recovery.

The existing `map_error_response()` in `crates/api/src/client.rs` maps HTTP 400 to a generic `ApiError::Api { status, message }` and does not distinguish prompt-too-long from other bad requests. HTTP 529 is also not handled — it likely falls through to the generic branch or a reqwest-level error.

The compaction service (`CompactionService`) has `compact`, `force_compact`, and `compact_if_needed` methods but no lightweight micro-compaction that strips tool results without calling the LLM.

## Goals / Non-Goals

**Goals:**
- Recover from API prompt-too-long errors by reactively compacting and retrying
- Provide a micro-compaction fallback that strips old tool results without an LLM call
- Auto-switch to a fallback model when the primary model is overloaded (consecutive 529s)
- Notify the user when model switching occurs
- Add proper error variant discrimination for prompt-too-long and overloaded responses

**Non-Goals:**
- Automatic model quality assessment after fallback (user manually switches back)
- Dynamic context window detection from API errors (we use the configured value)
- Retry budget sharing between reactive compaction and max-tokens recovery (they are independent)
- Persistent model switching — fallback is per-session only

## Decisions

### 1. Three-stage reactive compaction in agent_loop

When the API returns `PromptTooLong`:
- **Stage 1**: Full LLM-based compaction via existing `CompactionService::force_compact()`, then retry
- **Stage 2**: Micro-compaction (strip old tool results), then retry
- **Stage 3**: Report error to user, suggest manual `/compact`

**Rationale**: Staged escalation avoids expensive micro-compaction when regular compaction suffices, and gives the user a clear path when both fail. Using `force_compact()` instead of `compact_if_needed()` ensures compaction actually runs even if the threshold check says no (since the API already told us context is too large).

**Alternative considered**: Single compaction attempt then error — rejected because micro-compaction is cheap and provides meaningful token reduction for tool-heavy conversations.

### 2. RetryState struct tracks recovery across error types

A new `RetryState` struct in `agent_loop.rs` holds:
- `prompt_too_long_stage: u8` (0-3, reset each turn)
- `consecutive_overload_count: u32` (reset on success)

**Rationale**: Keeping state in a dedicated struct avoids proliferating loose variables in the loop and makes the retry policy testable. Each user turn resets `prompt_too_long_stage` since the context may have changed.

**Alternative considered**: Per-error counters as fields on `QueryLoop` — rejected because retry state is per-`run()` invocation, not per-QueryLoop lifetime.

### 3. Micro-compaction replaces old tool results with placeholder text

`micro_compact()` iterates message history and replaces `ToolResult` content blocks older than the most recent N turns (default 3) with `[Content cleared to reduce context size]`. Targets: Bash output, FileRead content, Grep/Glob results, WebSearch/WebFetch results.

**Rationale**: Tool results (especially Bash output and file contents) are the largest token consumers. Clearing old ones while preserving recent context gives significant reduction without losing the conversation's decision history. The placeholder text lets the model know content was removed.

**Alternative considered**: Truncating tool results to N characters — rejected because partial tool results can be misleading to the model.

### 4. Model fallback on consecutive 529 errors

After `MAX_OVERLOAD_RETRIES` (default 3) consecutive 529 errors, if `fallback_model` is configured, switch to it for the current session. Notify via `OutputSink::error()` or a new `OutputSink::model_switched()` method. Reset counter on any successful response.

**Rationale**: 529 errors mean the model is overloaded. Waiting indefinitely is a poor UX. A configured fallback lets users continue working. Auto-reset means the system naturally returns to the primary model in subsequent turns when load decreases.

**Alternative considered**: Exponential backoff without fallback — rejected because the user asked for fallback, and backoff alone can still leave users stuck for minutes.

### 5. New ApiError variants with structured parsing

Add `PromptTooLong { message: String }` and `Overloaded { message: String }` to `ApiError`. Update `map_error_response()` to detect:
- HTTP 400 + `error.type == "invalid_request_error"` + message containing "too long" or "too many tokens" → `PromptTooLong`
- HTTP 529 → `Overloaded`

**Rationale**: Structured variants enable pattern matching in the agent loop without string parsing. The HTTP 529 status code is unambiguous. For prompt-too-long, we check both the status and error type since HTTP 400 can mean many things.

### 6. fallback_model in Config with standard resolution chain

Add `fallback_model: Option<String>` to `Config`. Resolution: `RUST_CLAUDE_FALLBACK_MODEL` env → `settings.json fallbackModel` → config.json → `None`.

**Rationale**: Follows the established pattern for all other config fields. `Option<String>` means fallback is opt-in — no default fallback model is imposed.

## Risks / Trade-offs

- **[Risk] Reactive compaction may fail too** → Mitigation: Three stages with graceful degradation; stage 3 reports to user
- **[Risk] Micro-compaction removes useful context** → Mitigation: Preserve most recent 3 turns of tool results; the model can re-read files if needed
- **[Risk] Fallback model may have different capabilities** → Mitigation: User explicitly configures fallback_model; they choose what's acceptable
- **[Risk] 529 detection may vary across providers** → Mitigation: Only Anthropic API returns 529; Bedrock/Vertex use different error codes — this is Anthropic-specific for now
- **[Trade-off] Micro-compaction mutates message history in-place** → This is consistent with how regular compaction works. The alternative of creating a modified copy would double memory usage for large conversations
