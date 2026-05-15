# SessionEnd Hook Implementation Plan

## Summary

Add a first-class `SessionEnd` hook that runs during graceful thread shutdown, including when the user exits the TUI. It should use the existing configured-hook system, emit hook started/completed events, appear in hook listings/config schemas, and be best-effort: hook failures are reported but must not block session shutdown.

## Key Changes

- Add `SessionEnd` to the hook event surface: `HookEventName`, hook event name constants, hook key labels, analytics/metrics labels, app-server v2 hook enum, TUI hook display labels, and hook listing/browser support.
- Add config/schema support so hooks JSON/config can contain a `SessionEnd` event with command handlers.
- Add hook execution support in `codex-rs/hooks`, with command input fields `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `model`, and `permission_mode`.
- Wire shutdown behavior so `SessionEnd` runs once from core graceful shutdown before runtime teardown removes resources the hook may need. Also run it when the submission loop exits without explicit `Op::Shutdown`.
- Keep semantics best-effort: no additional model context injection, no continuation/block behavior, no shutdown cancellation.
- After implementation and verification, build the custom Codex CLI and copy it under `build/`.

## Test Plan

- Unit tests in `codex-hooks` for `SessionEnd` handler selection, command input serialization, empty output success, JSON output handling, and failed command handling.
- Config/discovery tests proving `SessionEnd` loads from hooks JSON/config, appears in hook listings, and uses stable persisted hook keys.
- Core shutdown tests proving graceful `Op::Shutdown` runs the hook exactly once and hook failure does not prevent `ShutdownComplete`.
- Regenerate hook schemas and app-server schemas when the protocol/config shape changes.
- Run `just fmt`, targeted crate tests, and scoped `just fix -p ...` for affected Rust crates.

## Manual Test Steps

1. Create a temporary hooks config with a `SessionEnd` command that writes stdin to a marker file.
2. Run the custom CLI from `build/` with hooks enabled and that config active.
3. Start a session, then use the normal exit path in the TUI.
4. Confirm the marker file exists and includes `hook_event_name: "SessionEnd"`, the active `session_id`, `cwd`, `model`, and nullable `transcript_path`.
5. Repeat with a nonzero hook command and confirm the CLI still exits cleanly.

## Assumptions

- `SessionEnd` is thread-scoped like `SessionStart`.
- `SessionEnd` matchers are ignored, matching current `Stop` and `UserPromptSubmit` behavior.
- Only command handlers are executable in this hook engine.
- The user's exit command means the normal graceful TUI shutdown path that submits `Op::Shutdown`.
