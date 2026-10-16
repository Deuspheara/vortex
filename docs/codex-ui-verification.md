# Codex native UI verification — 2026-10-16

The earlier backend tests did not establish that the desktop app worked. This
pass exercised the normal build, with the installed Codex CLI 0.157.1 and the
existing local login, through the native GPUI composer and approval controls.
Normal-app QA also used an isolated SQLite store with a deterministic protocol
peer for controlled failure states. Demo mode was used only for its fixture test.

## Failures found and corrected

- The absolute composer overlapped the virtual thread's hit-testing area. Typing
  and model selection were unreachable despite a connected backend. The footer
  now participates in normal flex layout with a definite input height. Invisible
  dock targets exist only while dragging.
- Finder's PATH could miss Codex and the Node interpreter used by npm launchers.
  Discovery now covers common installation locations, preserves explicit
  overrides, and supplies the launcher's directory to its child PATH.
- First-run provider selection hid Codex sign-in errors behind Mock. New empty
  sessions show Codex setup guidance, while explicit preferences and existing
  session backend identities remain intact. Sign-in polling releases the status
  lock before refreshing.
- Session replay sorted per-run sequence numbers across all runs. It now follows
  durable insertion order. Focus mode retains the settled reply in each turn.
- Deferred full snapshots and queued item patches could restore obsolete rows or
  running labels. Structural snapshots commit immediately and invalidate older
  patches; token updates still coalesce per frame with sealed streaming caches
  and virtualized stable history.
- Codex add/delete items contain literal file contents. Normalization now creates
  proper unified diffs; deleted files retain their original path. Real new-file
  previews now show the correct line counts.
- Approval decisions now persist with their request and tool identity. Concurrent
  requests in one turn are presented sequentially. Unsupported interactions
  receive a protocol error and stop the affected turn visibly.
- Cancellation waits for confirmation, terminates unfinished tool projections,
  and leaves a replayable stopped notice. Background sessions cannot replace the
  selected session's approval or Stop controls.
- Error text wraps within the thread, the changes panel keeps its review width,
  and its approval footer fits below the inspector tabs.
- Closing the last native window previously left a headless application alive.
  Last-window shutdown, standard Quit/Close Window menus and shortcuts, and
  foreground activation now follow the normal desktop lifecycle.

## Native application checks

| State | Exercise and observed result |
| --- | --- |
| Empty | Fresh normal-app store boots into a usable empty thread. |
| Discovery/auth/models | Real app launched with `PATH=/usr/bin:/bin`, without `VORTEX_CODEX_BIN`; NVM Codex and Node were discovered, local login reused, and model/effort options populated. |
| Streaming/completed | Typed a tiny marker request into the native composer; streamed reply completed. |
| Approval preview | Requested one disposable `/tmp` file with `apply_patch`; native card showed the path and changes panel showed `+1` before permission. |
| Rejection | Clicked Deny; file stayed absent and the settled response reported rejection. |
| Acceptance/diff review | Repeated the request and clicked Allow once; exact requested contents appeared, and the completed diff was marked Applied. |
| Output/cancellation | Approved a harmless `printf; sleep; printf` command, then clicked Stop; cancellation completed and the stopped notice survived reopening. |
| Real reconnect | Terminated only Vortex's owned app-server while an edit awaited permission. It reconnected, expired the approval, stopped the uncertain turn, and left the unapproved file absent. Manually submitted follow-ups completed. |
| Persistence | Reopened Vortex repeatedly; earlier replies, denied edit, applied diff, and stopped turns remained, using the same Vortex sessions and Codex thread. No interrupted turn was automatically resent. |
| Normal quit/close | Invoked Quit Vortex from the native menu, reopened, then closed the final native window. Both paths exited successfully and process inspection found no remaining Vortex-owned app-server. |
| Sign-in recovery | Normal UI with isolated peer showed `codex login` guidance, then populated models after simulated sign-in without restarting. Real credentials were untouched. |
| Missing CLI | Isolated normal launch with an invalid executable showed installation/`VORTEX_CODEX_BIN` guidance and an unavailable model state, without silently using Mock. |
| Concurrent approvals | Isolated peer sent two requests together; native card presented `echo 1`, then `echo 2` after the first decision, then returned to an idle composer with one completed reply. |
| Failure/unsupported | Isolated peer requested unsupported permissions; UI displayed a wrapped error, returned to an idle composer, and could accept another turn. |
| Peer reconnect | Peer exited after a partial response; UI showed recovery then one reconciled `hello world`, without duplicate text or automatic resubmission. |

## Native UI refinement

The website reference alone was insufficient. The installed **reference application**
was inspected directly, including an existing conversation, the empty draft,
Appearance settings, the right-panel surface chooser, and General → Legacy
features. It supports both the flat sidebar and per-project thread trees; the
latter is exposed as “Sidebar (legacy)”. Vortex's project tree remains intact.
The installed reference uses Catppuccin Frappé, SF Pro at 16 px, and Comfortable
chat width. The bundled Vortex dark palette and layout now follow that visible
setup while retaining Vortex branding and its stricter execution defaults.

The follow-up revision adds:

- A 296 px project sidebar with a working inline conversation filter and compact
  new-thread action, plus headers owned by their actual resizable panes.
- A project/thread breadcrumb, 52 px headers, 736 px chat width, flat activity
  rows, and compact right-aligned user bubbles with font-metric sizing.
- A centered empty-thread composer that moves to the bottom for existing chats.
  Model, advertised reasoning effort, permissions, attachments, and send/stop
  controls share one rounded surface; branch/context metadata sits below it.
- Separate Codex model/effort controls that preserve the exact advertised routing
  values. Codex still offers only read-only Plan and approval-required execution.
- A panel chooser for Changes, Context, Plan, Terminal, and Android. Tabs open on
  demand; selecting an existing surface preserves its identity and dock.
- Semantic attachment, branch, permission, and new-thread glyphs; readable
  animated activity text; menus that dismiss on selection; full-input-area focus.
- A long-path approval footer that keeps the complete path and its buttons
  visible, readable diff filenames, a diff list constrained below its header,
  and a single-line diff summary matching its virtualized row height.

Normal native-app checks in this follow-up used both the isolated deterministic
peer and the real installed Codex CLI with existing authentication:

| Check | Observed result |
| --- | --- |
| Real inference | `gpt-6.1-sol` completed a tiny marker prompt through the native composer. |
| Effort routing | Selecting Medium in the separate control dismissed the menu; the next real edit ran with the selected advertised effort. |
| Real edits | Two `apply_patch` requests displayed native approvals and previews. Allow once produced the exact disposable file contents, then the expected `+1/−1` replacement diff. |
| Approval layout | Long absolute path, risk label, and both review-panel buttons remained visible together. |
| Rejection | The controlled peer's approval was denied through the native card; the UI settled and remained usable. |
| Replay | Reopening the same isolated store retained real replies and the applied diff without duplicated text or resubmission. |
| Navigation | Inline sidebar filtering removed nonmatching threads in place; the panel chooser opened Context and dismissed correctly. |
| Responsive controls | Opening the changes panel preserved input, model/effort, permission, and attachment controls; controls wrap instead of being hidden. |

Native reference screenshots and QA artifacts remain outside the repository.
This is a substantial visual follow-up, not a claim of pixel-identical output.
Further narrow-pane transcript sizing and matching-window comparisons remain
part of the ongoing UI verification.

## Verification commands

- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets` — passed with repository warnings.
- `cargo check -p app` — passed with repository warnings.
- `cargo test --workspace` — 236 passed, 2 opt-in live tests ignored.
- `cargo test -p app --features demo_mode fixture_is_isolated_idempotent_and_replayable`
  — passed.
- `VORTEX_TEST_CODEX_MODEL=gpt-6-astra cargo test -p agent_codex --test backend -- --ignored --nocapture --test-threads=1`
  — both live tests passed, including approval-before-edit and context retention
  after restarting the backend. An earlier concurrent invocation using the old
  test model preference encountered an upstream HTTP 429 on its second inference;
  that attempt was not counted as passing.

Regression coverage includes multi-run replay order, reasoning reconciliation,
new/deleted file diffs, sign-in recovery, correlated decisions, queued concurrent
approvals, cancellation, and safe unsupported-request failure.

## Limits

OpenRouter was preserved and workspace-tested, but no live OpenRouter inference
was exercised in this pass. The protocol remains version-sensitive. Unsupported
interactive requests stop visibly instead of being guessed. External commands
may change files without a file-change item: Codex execution does not acquire
Vortex's native universal patch-before-write or crash rollback guarantees.

Native screenshots and runtime stores were kept outside the repository; no
credentials, logs, local SQLite data, or generated binaries are committed.
