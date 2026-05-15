 Implemented SessionEnd hook support and did not commit or push.

What changed:

- Added SessionEnd as a hook event across protocol/config/hooks/app-server/TUI/analytics.
- Added SessionEnd hook input/output schemas.
- Wired shutdown to run SessionEnd hooks before conversation/MCP teardown.
- Added unit/integration coverage for config, hook parsing, and shutdown execution.
- Wrote the implementation plan at analysis/session-end-hook-plan.md.
- Built the custom CLI at build/codex-session-end.

Validation:

- Passed: cargo test -p codex-hooks
- Passed: cargo test -p codex-config
- Passed: cargo test -p codex-app-server-protocol
- Passed: cargo test -p codex-core session_end_hook --test all
- Passed: cargo test -p codex-analytics
- Passed focused TUI hook snapshots after accepting the intended SessionEnd snapshot.
- Passed focused app-server config requirement test.
- codex-app-server full crate and codex-tui full crate both aborted in unrelated stack-overflowing tests.
- just fmt ran Rust formatting, then failed in the Python SDK formatter because openai-codex-cli-bin has no wheel for this glibc Linux platform.
- git diff --check passed.
- Release build passed; binary reports codex-cli 0.0.0.

Git remotes are now:

github  org-14957082@github.com:openai/codex.git (fetch)
github  DISABLED (push)
origin  https://gitea.antonysallas.com/asallas/codex.git (fetch)
origin  https://gitea.antonysallas.com/asallas/codex.git (push)

To test /exit triggers SessionEnd:

cd /home/asallas/workarea/projects/personal/codex

TEST_HOME="$(mktemp -d)"
rsync -a ~/.codex/ "$TEST_HOME"/
LOG="$TEST_HOME/session-end.jsonl"
HOOK="$TEST_HOME/session-end-hook.py"

cat > "$HOOK" <<'PY'
#!/usr/bin/env python3
import json, os, sys
payload = json.load(sys.stdin)
with open(os.environ["SESSION_END_LOG"], "a", encoding="utf-8") as f:
    f.write(json.dumps(payload, sort_keys=True) + "\n")
print(json.dumps({"suppressOutput": True}))
PY
chmod +x "$HOOK"

cat >> "$TEST_HOME/config.toml" <<EOF

[[SessionEnd]]
[[SessionEnd.hooks]]
command = "SESSION_END_LOG=$LOG python3 $HOOK"
EOF

CODEX_HOME="$TEST_HOME" ./build/codex-session-end --dangerously-bypass-hook-trust

In the TUI, type:

/exit

After Codex exits:

cat "$LOG"

Expected: one JSON line with "hook_event_name":"SessionEnd" plus fields like session_id, cwd, transcript_path, model, and permission_mode.

cat > "$HOOK" <<'PY'
#!/usr/bin/env python3
import json, os, sys
payload = json.load(sys.stdin)
with open(os.environ["SESSION_END_LOG"], "a", encoding="utf-8") as f:
    f.write(json.dumps(payload, sort_keys=True) + "\n")
print(json.dumps({"suppressOutput": True}))
PY