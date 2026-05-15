use std::path::PathBuf;

use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookRunSummary;
use codex_utils_absolute_path::AbsolutePathBuf;

use super::common;
use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::dispatcher;
use crate::engine::output_parser;
use crate::schema::SessionEndCommandInput;

#[derive(Debug, Clone)]
pub struct SessionEndRequest {
    pub session_id: ThreadId,
    pub cwd: AbsolutePathBuf,
    pub transcript_path: Option<PathBuf>,
    pub model: String,
    pub permission_mode: String,
}

#[derive(Debug)]
pub struct SessionEndOutcome {
    pub hook_events: Vec<HookCompletedEvent>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct SessionEndHandlerData;

pub(crate) fn preview(
    handlers: &[ConfiguredHandler],
    _request: &SessionEndRequest,
) -> Vec<HookRunSummary> {
    dispatcher::select_handlers(
        handlers,
        HookEventName::SessionEnd,
        /*matcher_input*/ None,
    )
    .into_iter()
    .map(|handler| dispatcher::running_summary(&handler))
    .collect()
}

pub(crate) async fn run(
    handlers: &[ConfiguredHandler],
    shell: &CommandShell,
    request: SessionEndRequest,
) -> SessionEndOutcome {
    let matched = dispatcher::select_handlers(
        handlers,
        HookEventName::SessionEnd,
        /*matcher_input*/ None,
    );
    if matched.is_empty() {
        return SessionEndOutcome {
            hook_events: Vec::new(),
        };
    }

    let input_json = match serde_json::to_string(&SessionEndCommandInput::new(
        request.session_id.to_string(),
        request.transcript_path.clone(),
        request.cwd.display().to_string(),
        request.model.clone(),
        request.permission_mode.clone(),
    )) {
        Ok(input_json) => input_json,
        Err(error) => {
            return SessionEndOutcome {
                hook_events: common::serialization_failure_hook_events(
                    matched,
                    /*turn_id*/ None,
                    format!("failed to serialize session end hook input: {error}"),
                ),
            };
        }
    };

    let results = dispatcher::execute_handlers(
        shell,
        matched,
        input_json,
        request.cwd.as_path(),
        /*turn_id*/ None,
        parse_completed,
    )
    .await;

    SessionEndOutcome {
        hook_events: results.into_iter().map(|result| result.completed).collect(),
    }
}

fn parse_completed(
    handler: &ConfiguredHandler,
    run_result: CommandRunResult,
    turn_id: Option<String>,
) -> dispatcher::ParsedHandler<SessionEndHandlerData> {
    let mut entries = Vec::new();
    let mut status = HookRunStatus::Completed;

    match run_result.error.as_deref() {
        Some(error) => {
            status = HookRunStatus::Failed;
            entries.push(HookOutputEntry {
                kind: HookOutputEntryKind::Error,
                text: error.to_string(),
            });
        }
        None => match run_result.exit_code {
            Some(0) => {
                let trimmed_stdout = run_result.stdout.trim();
                if !trimmed_stdout.is_empty() {
                    if let Some(parsed) = output_parser::parse_session_end(&run_result.stdout) {
                        if let Some(system_message) = parsed.universal.system_message {
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Warning,
                                text: system_message,
                            });
                        }
                        let _ = parsed.universal.suppress_output;
                        if !parsed.universal.continue_processing {
                            status = HookRunStatus::Stopped;
                            if let Some(stop_reason_text) = parsed.universal.stop_reason {
                                entries.push(HookOutputEntry {
                                    kind: HookOutputEntryKind::Stop,
                                    text: stop_reason_text,
                                });
                            }
                        }
                    } else if output_parser::looks_like_json(&run_result.stdout) {
                        status = HookRunStatus::Failed;
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Error,
                            text: "hook returned invalid session end JSON output".to_string(),
                        });
                    } else {
                        entries.push(HookOutputEntry {
                            kind: HookOutputEntryKind::Warning,
                            text: trimmed_stdout.to_string(),
                        });
                    }
                }
            }
            Some(exit_code) => {
                status = HookRunStatus::Failed;
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: format!("hook exited with code {exit_code}"),
                });
            }
            None => {
                status = HookRunStatus::Failed;
                entries.push(HookOutputEntry {
                    kind: HookOutputEntryKind::Error,
                    text: "hook exited without a status code".to_string(),
                });
            }
        },
    }

    let completed = HookCompletedEvent {
        turn_id,
        run: dispatcher::completed_summary(handler, &run_result, status, entries),
    };

    dispatcher::ParsedHandler {
        completed,
        data: SessionEndHandlerData,
        completion_order: 0,
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::protocol::HookEventName;
    use codex_protocol::protocol::HookRunStatus;
    use codex_protocol::protocol::HookSource;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::SessionEndHandlerData;
    use super::parse_completed;
    use crate::engine::ConfiguredHandler;
    use crate::engine::command_runner::CommandRunResult;

    #[test]
    fn parse_completed_accepts_empty_success() {
        let parsed = parse_completed(
            &handler(),
            CommandRunResult {
                exit_code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
                error: None,
                started_at: 10,
                completed_at: 12,
                duration_ms: 2,
            },
            /*turn_id*/ None,
        );

        assert_eq!(parsed.completed.run.status, HookRunStatus::Completed);
        assert_eq!(parsed.completed.turn_id, None);
        assert_eq!(parsed.data, SessionEndHandlerData);
    }

    #[test]
    fn parse_completed_reports_invalid_json() {
        let parsed = parse_completed(
            &handler(),
            CommandRunResult {
                exit_code: Some(0),
                stdout: r#"{"continue":false"#.to_string(),
                stderr: String::new(),
                error: None,
                started_at: 10,
                completed_at: 12,
                duration_ms: 2,
            },
            /*turn_id*/ None,
        );

        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries[0].text,
            "hook returned invalid session end JSON output"
        );
    }

    #[test]
    fn parse_completed_records_stop_json_without_blocking_shutdown() {
        let parsed = parse_completed(
            &handler(),
            CommandRunResult {
                exit_code: Some(0),
                stdout: r#"{"continue":false,"stopReason":"done"}"#.to_string(),
                stderr: String::new(),
                error: None,
                started_at: 10,
                completed_at: 12,
                duration_ms: 2,
            },
            /*turn_id*/ None,
        );

        assert_eq!(parsed.completed.run.status, HookRunStatus::Stopped);
        assert_eq!(parsed.completed.run.entries[0].text, "done");
    }

    #[test]
    fn parse_completed_reports_nonzero_exit() {
        let parsed = parse_completed(
            &handler(),
            CommandRunResult {
                exit_code: Some(1),
                stdout: String::new(),
                stderr: String::new(),
                error: None,
                started_at: 10,
                completed_at: 12,
                duration_ms: 2,
            },
            /*turn_id*/ None,
        );

        assert_eq!(parsed.completed.run.status, HookRunStatus::Failed);
        assert_eq!(
            parsed.completed.run.entries[0].text,
            "hook exited with code 1"
        );
    }

    fn handler() -> ConfiguredHandler {
        ConfiguredHandler {
            event_name: HookEventName::SessionEnd,
            matcher: None,
            command: "echo ok".to_string(),
            timeout_sec: 5,
            status_message: None,
            source_path: test_path_buf("/tmp/hooks.json").abs(),
            source: HookSource::User,
            display_order: 0,
            env: std::collections::HashMap::new(),
        }
    }
}
