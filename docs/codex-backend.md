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

Discovery checks PATH, common local npm/Volta/Homebrew directories, and installed
NVM versions (newest version first). npm launchers also receive Node's directory
on their child PATH, including when Vortex starts from Finder. Set
`VORTEX_CODEX_BIN` to an executable path to override discovery; an invalid override
fails visibly rather than silently selecting another CLI.
Choose **Codex** under Settings → Models. The composer has separate model and reasoning selectors populated with the
choices advertised by the installed CLI. Availability comes from `model/list`,
not a hard-coded model catalog. New installations select Codex and show explicit
installation/sign-in/connection guidance until ready;
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
requests; unsupported interactions stop the affected turn with a visible error.
Command/edit requests within a turn are presented in order, one at a time.
Queued requests persist a normalized waiting state for their tool item without
replacing the active decision card.
Allow/deny decisions are persisted as `ApprovalResolved` events correlated to the
exact tool item. Approvals expire when a process is replaced. Vortex never interprets
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
before publication to the UI. Session replay uses database insertion order,
not per-run sequence numbers, so separate turns and child runs cannot interleave
incorrectly. Completed runs are not re-imported on later resumes.

Broken connections retry with delays of 1, 2, 4, and 8 seconds, then report an
unavailable state. Restart Vortex to retry after exhaustion. Sign-in-required
state rechecks every 15 seconds. Previously confirmed turn IDs are reconciled
from stored Codex history; only missing text suffixes and new tool completions
are emitted. Vortex never automatically resends a turn whose outcome is
uncertain. Interrupted work is reported explicitly; review changes and send a
follow-up. Cancellation uses `turn/interrupt`; the UI keeps the turn active until
completion confirms interruption. Unfinished tool items receive terminal events,
and a stopped notice survives replay.

The protocol is version-sensitive. Integration was exercised with Codex CLI
0.157.1; an incompatible CLI produces an actionable error instead of silently
falling back to Mock. The first implementation intentionally rejects unsupported
server interaction types rather than guessing their approval semantics.

## Context meter and token accounting

Context occupancy and processed usage are separate quantities. Codex sends
`thread/tokenUsage/updated` with thread-cumulative `total`, latest-request `last`,
and `modelContextWindow`. Vortex persists `last.totalTokens` and the reported
capacity as `ContextWindowUpdated`; the composer ring and fresh-context advice
use that snapshot. Compaction can decrease the snapshot. Cumulative `total`
deltas remain `UsageUpdated` for the run's input/output/cache ledger; cache hits
are included in context occupancy and are not subtracted from the context window.

The native backend uses its latest request usage (or the ContextBuilt estimate
before usage arrives), rather than adding requests together for the ring. Its
capacity still comes from the model catalog. Codex capacity comes from the server,
without guessing a 200K fallback. When an older saved Codex run predates occupancy
snapshots, the meter says context usage is unavailable until a new usage update;
existing token totals are retained and no prompt is automatically resubmitted.

An investigation of the saved “make a simple html js todo app” run found five
model calls: 102,469 accumulated input tokens, including 69,248 cache-read tokens,
and 2,419 output tokens. The final request occupied 22,098 / 475,000 tokens (4.65%).
The old meter incorrectly displayed cumulative input against a 200,000 fallback
(51.23%). The reference adapter uses `last.totalTokens` and
`modelContextWindow`, matching the corrected projection. This is a reporting fix,
not a claim that the underlying model requests became cheaper or smaller.

The Codex path sends the new user input to the existing Codex thread; it does not
inject Vortex's native context builder, transcript, project map, or tool catalog.
Installed Codex instructions/plugins and model behavior can still affect actual
usage. A performance comparison requires matching model, effort, workspace,
configuration, and task; no equivalent live reference application benchmark was run here.

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

For reproducible native UI testing, build the normal app and run
`python3 scripts/run-codex-ui-test.py`. This creates an isolated store and workspace
and reuses the installed CLI's authentication without copying it. `--fixture`
selects the deterministic protocol peer; `--fixture --signed-out` simulates missing
authentication without changing the user's real account. `--missing-cli` tests
installation guidance. The normal app also accepts an absolute `VORTEX_DATA_DIR`
for an isolated store. For reproducible native layout checks, add
`--window-size 1100x800` to the QA launcher. This debug-only size override requires
an isolated data directory and does not select demo mode. Demo mode keeps its separate marker requirement and never
starts Codex. See [native verification results](codex-ui-verification.md).
