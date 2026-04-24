## ADDED Requirements

### Requirement: Token-by-token stdout streaming in print mode
The CLI SHALL stream text content token-by-token to stdout in `--print` mode as `StreamDelta` events arrive, rather than collecting the full response before outputting.

#### Scenario: Text appears progressively in print mode
- **WHEN** the CLI is invoked with `--print` and the model streams a response
- **THEN** each text delta SHALL be written to stdout and flushed immediately so the user sees progressive output

#### Scenario: Non-streaming fallback in print mode
- **WHEN** the CLI is invoked with `--print --no-stream`
- **THEN** the full response SHALL be written to stdout after completion (existing behavior preserved)

### Requirement: Thinking content suppressed in print mode
The CLI SHALL NOT write thinking/reasoning content to stdout in `--print` mode since print mode output is intended for machine consumption or piping.

#### Scenario: Thinking deltas not written to stdout
- **WHEN** the model produces thinking content followed by response text in `--print` mode
- **THEN** only the response text SHALL appear on stdout; thinking content SHALL be suppressed

### Requirement: Tool events on stderr in print mode
The CLI SHALL write tool use and tool result summaries to stderr in `--print` mode so they are visible to the user but do not pollute the piped stdout stream.

#### Scenario: Tool use logged to stderr
- **WHEN** a tool call is executed during `--print` mode
- **THEN** a brief summary line (tool name + key parameters) SHALL be written to stderr

#### Scenario: Tool result logged to stderr
- **WHEN** a tool returns a result during `--print` mode
- **THEN** a brief result summary (success/error + truncated output) SHALL be written to stderr

### Requirement: Print mode interrupt cleanup
The CLI SHALL handle Ctrl+C during `--print` mode streaming by flushing any pending output and exiting with a non-zero status code.

#### Scenario: Ctrl+C during print streaming
- **WHEN** the user presses Ctrl+C while `--print` mode is streaming output
- **THEN** the CLI SHALL flush stdout, write a newline, and exit with status code 130 (standard SIGINT convention)
