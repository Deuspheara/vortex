# Local Codex backend

Vortex supports two execution backends. The native backend uses Mock or
OpenRouter with Vortex's own model loop, tool validation, patch transactions,
and approval policy. The Codex backend runs the installed `codex app-server`
agent over local JSONL stdio. Codex owns its model context, tools, sandbox, and
file writes; Vortex owns the native interface and approval responses.

## Setup and daily use

Install the official Codex CLI and run `codex login` once. Launch Vortex normally
with `cargo run -p app`. No OpenRouter API key is needed for Codex. Vortex asks
app-server for account status; it does not read, copy, log, or store auth tokens.
Codex handles its own saved authentication and token refresh. Expired or revoked
credentials can still require another login.

If Codex is not on the app's PATH, set `VORTEX_CODEX_BIN` to its executable path.
Choose **Codex** under Settings → Models. The model picker includes the reasoning
levels advertised by the installed CLI. Availability comes from `model/list`,
not a hard-coded model catalog. New installations prefer authenticated Codex;
the last selected provider is saved. Demo builds never start Codex.

A conversation retains its backend after its first run. Start a new conversation
to switch between Codex and the native agent. Existing Vortex conversations stay
on the native backend. Vortex does not automatically import other CLI or desktop
chats. Text prompts and image attachments are sent through app-server; Vortex
keeps text history while Codex owns the complete provider conversation.

## Permissions and edit review

Codex sessions explicitly start read-only with `untrusted` command approvals,
user-reviewed approval routing, disabled web search, and no sandbox network
access. Apply mode allows Codex to request approval for a command or file edit.
Plan mode uses read-only execution with escalation disabled. Vortex does not
inherit a full-access CLI default or expose a full-access Codex mode.

The user's approval response is correlated to the exact server request. Unknown
server requests are rejected, including unsupported permission and credential
requests. Approvals expire when a process is replaced. Vortex never interprets
"always allow" as an unbounded Codex permission grant.

Codex file-change items supply previews and completed diffs. Completed edits are
shown as observed changes, not as native Vortex patches awaiting application.
**The native backend's universal patch-before-write guarantee does not apply to
Codex.** Shell commands and external tools can change files without a patch item;
users should also inspect their working-tree diff. Vortex does not claim atomic
rollback or exhaustive crash-time change recovery for external execution.

## Lifecycle and persistence

One app-server process is owned by each Vortex app instance. It initializes once,
serves multiple conversations, and is killed when its owner shuts down. There is
no TCP listener or daemon that continues running after Vortex quits.

SQLite stores session backend, Codex thread ID, model/effort, normalized
`AgentEvent`s, and per-run reconciliation state. The additive migration leaves
legacy sessions untouched. Provider events never reach GPUI directly. Each
normalized event batch and its reconciliation cursor commit in one transaction
before publication to the UI.

Broken connections retry with delays of 1, 2, 4, and 8 seconds, then report an
unavailable state. Restart Vortex to retry after exhaustion. Sign-in-required
state rechecks every 15 seconds. Previously confirmed turn IDs are reconciled
from stored Codex history; only missing text suffixes and new tool completions
are emitted. Vortex never automatically resends a turn whose outcome is
uncertain. Interrupted work is reported explicitly; review changes and send a
follow-up. Cancellation uses `turn/interrupt` and waits for completion events.

The protocol is version-sensitive. Integration was exercised with Codex CLI
0.157.1; an incompatible CLI produces an actionable error instead of silently
falling back to Mock. The first implementation intentionally rejects unsupported
server interaction types rather than guessing their approval semantics.

## Verification

- `cargo test -p agent_codex`: deterministic local fake-server tests; no network
  or account required. Covers streaming, persistence, resume, approval/rejection,
  unknown requests, cancellation, sign-out, model errors, and process crashes.
- `cargo test -p agent_store`: additive migration, persistence and atomic
  event/cursor tests.
- `cargo test -p agent_codex --test backend live_codex_login_and_conversation_survive_backend_restart -- --ignored --nocapture`:
  explicitly opt-in test using the local account for two tiny inference turns in
  a temporary directory. It creates a test thread in Codex's own history.

- `cargo test -p agent_codex --test backend live_codex_edit_requires_approval_and_reports_completed_diff -- --ignored --nocapture`:
  opt-in local-account test that checks a temporary file remains unchanged until
  approval and then verifies the completed diff.

Reference: [official app-server documentation](https://developers.openai.com/codex/app-server).
