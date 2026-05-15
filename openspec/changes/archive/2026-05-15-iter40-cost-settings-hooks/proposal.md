## Why

迭代 40 补齐三个已经在第四期规划中明确的工程缺口：成本估算目前过于粗略，配置演进缺少可执行迁移机制，Hook 系统缺少会话生命周期事件与一次性执行控制。现在补齐这些能力，可以让后续认证、沙箱和 SDK 工作建立在更稳定的运行时基础上。

## What Changes

- Add model-aware cost calculation with separate input, output, cache-read, and cache-creation pricing.
- Add session cost tracking and budget status checks that can warn when configured budget limits are exceeded.
- Add a config migration runner with versioned migrations stored in `config.json`.
- Add startup integration so config migrations run before config values are used.
- Extend hook configuration with `once` support.
- Add `SessionStart` and `SessionEnd` hook events with structured JSON payloads.
- Wire CLI session lifecycle points to emit the new hook events.

## Capabilities

### New Capabilities
- `cost-tracking`: Model-aware usage cost calculation, session cost accumulation, and budget status reporting.
- `settings-migration`: Versioned configuration migrations that upgrade persisted config files before use.

### Modified Capabilities
- `hook-config`: Hook definitions support a `once` flag that limits a hook command to one execution per session.
- `hook-execution`: Hook execution supports `SessionStart` and `SessionEnd` lifecycle events with event-specific JSON input.
- `slash-command-extensions`: Slash command handling exposes detailed cost reporting through `/cost`.

## Impact

- Affected crates: `core`, `cli`, and possibly `sdk` if hook types are re-exported or mirrored there.
- Affected runtime paths: config loading, query-loop usage accounting, slash command dispatch, and CLI session startup/shutdown.
- No breaking CLI arguments are expected.
