How SessionStart Works
  SessionStart is a first-class hook event in the hook event enum, but there is no matching SessionEnd hook event today: codex-rs/protocol/src/protocol.rs:1469.

  SessionStart is configured through hooks JSON/config under the SessionStart key: codex-rs/config/src/hook_config.rs:30. It supports a matcher, but the matcher is matched against the
  session-start source: startup, resume, or clear: codex-rs/hooks/src/events/session_start.rs:21.

  At session initialization, core maps the initial history to that source and stores it as pending state: codex-rs/core/src/session/session.rs:1116. It is not run immediately at thread
  construction. Instead, the first turn consumes the pending source via run_pending_session_start_hooks: codex-rs/core/src/hook_runtime.rs:109. The turn loop checks it before normal user
  prompt/model work: codex-rs/core/src/session/turn.rs:308.

  The hook command receives JSON with session_id, transcript_path, cwd, hook_event_name, model, permission_mode, and source: codex-rs/hooks/src/schema.rs:396. Output can stop further
  processing or inject additional context into the model context: codex-rs/hooks/src/events/session_start.rs:165. Core emits HookStarted before execution and HookCompleted afterward:
  codex-rs/core/src/hook_runtime.rs:469.

  Does SessionEnd Exist?
  No hook-level SessionEnd exists. rg only found session_end in the TUI session log path, for JSONL logging, not hook dispatch: codex-rs/tui/src/session_log.rs:216.

  There is also a Stop hook, but that is turn-level: it runs when a model turn is about to stop, not when the whole session/thread shuts down.

  Implementation Plan

  1. Add SessionEnd to the shared hook event surface:
     codex-rs/protocol/src/protocol.rs, app-server v2 hook enum, hook analytics labels, TUI hook labels, and generated schemas/SDK artifacts.
  2. Add config support:
     extend HookEventsToml, managed hooks requirements, hook key labels, HOOK_EVENT_NAMES, and related list/discovery paths so SessionEnd can be declared in hooks JSON/config.
  3. Add hook schema and event module:
     create codex-rs/hooks/src/events/session_end.rs, plus SessionEndCommandInput and output schema. I’d keep the first version simple and thread-scoped:
     session_id, transcript_path, cwd, hook_event_name, model, permission_mode, and maybe an reason/source field if product semantics need it.
  4. Decide shutdown semantics before coding:
     I recommend SessionEnd should be best-effort and non-blocking for shutdown semantics: failures are reported as hook failures, but they should not prevent shutdown. Unlike
     SessionStart, it should not inject model context.
  5. Wire execution into shutdown:
     call it from core shutdown before tearing down services that hooks may need, likely in the shutdown path around codex-rs/core/src/session/handlers.rs:627. Ensure it runs exactly once
     for both explicit Op::Shutdown and submission-loop termination.
  6. Add tests:
     hook parser/schema tests in codex-hooks, config/discovery tests, core shutdown test proving the hook runs once, and app-server/TUI snapshot or list tests if visible hook lists change.
  7. Regenerate artifacts and validate:
     run just write-hooks-schema, app-server schema generation if v2 protocol changes, just fmt, targeted tests for codex-hooks, codex-core, and app-server protocol if touched.