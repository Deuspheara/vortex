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
The responsive follow-up compared the installed app and Vortex at approximately
1305×964, and exercised a 400 px chat pane in a 1100×800 native window. It found
and fixed clipped expanded prompts, fixed-character row estimates, bottom-scroll
misclassification, multiline input/toolbar overlap, and a full-height table wrapper
that squeezed code blocks. Visible user/assistant rows now cache width-aware
heights and publish changes after layout; stable history stays virtualized and
the streaming caches remain in place. Code and table content keeps its natural
height and supports horizontal scrolling. Empty-state spacing follows the native
reference, and breadcrumbs remain readable in a narrow pane.

Real Codex checks verified expanded `USER-TAIL`, completed `RESPONSE-END`, a
heading/list/four-line Rust block/table ending with `STRUCTURE-END`, and a rejected
file edit at the minimum chat width. The file stayed absent before and after
rejection; the full path and decision controls remained visible. Reopening retained
the reply and the diff's explicit “Not applied” state. A long multiline draft kept
its final line above the wrapped toolbar. The deterministic peer also exercised
Stop, visible unsupported-request failure, sequential approvals, and reconnect
from partial `hello` to one reconciled `hello world` without resubmission.

The final queue check found and fixed a normalized-state issue: approvals queued
behind the active prompt now persist a waiting state for their exact tool item
without replacing the active card. Native checks verified both waiting rows,
cancellation clearing both requests, and sequential Allow once decisions. Reopening
retained the settled conversation without an orphaned approval. A decorative
transcript fade was also intercepting approval-card clicks; it no longer blocks
mouse input. Cancelled/failed command rows display “Stopped” rather than “Ran”.
Backend event changes and visual fixes are committed separately.

## Thread typography and activity follow-up

Inspected populated conversations and expanded work logs in the installed reference application,
 including its code-block treatment, and compared its implementation reference.
The previous shell pass left transcript components on inconsistent size scales.
This follow-up uses the macOS system reading font for interface/prose, 14 px body
and activity text, 12 px metadata, and 13 px Menlo for code/output/diffs. The terminal
retains its bundled Nerd Font. Transcript headings use explicit 20/18/16 px roles.

Reasoning and tool rows share a 24 px rhythm, a semantic 16 px line-icon column,
regular-weight labels, and proper disclosure chevrons. Duplicate backend labels,
parent-directory metadata, and the Codex composer activity strip were removed.
Commands disclose output inline without opening an inspector tab. The thread menu
can toggle activity details immediately, including on an idle conversation.
Code previews share one inset surface, language/copy header, border, padding, font,
and line height in both streaming and completed renderers. History remains virtual
and streaming keeps the cached sealed-block/hot-tail path.

Native checks at 1305×964 and 1100×800 (including a 400 px chat pane) verified:

- Real Codex ran only the requested bounded `printf` command after Allow once;
  its output contained `alpha` and `beta`, followed by a four-line Rust preview
  and `THREAD-STYLE-END`. The local authentication and model discovery were reused.
- Command rows collapse/expand inline; code, headings, prose, and the adjacent
  observed-change panel remain readable at narrow widths.
- Controlled `ui-thinking` updates appear immediately from a single reasoning
  delta, expand into wrapped text, and settle on Stop. The extra reducer buffer
  no longer holds a lone delta until another event.
- Controlled `ui-stream` retains the first prose paragraph and every line of an
  open code fence, including `STREAM-TAIL-LINE`, while still streaming. The first
  sealed-prefix parse is scheduled immediately; a pending parse cannot hide prose.
  A regression test covers the initial-prefix/cached-tail transition.
- Activity details toggle on completed history without requiring another event.
  Short user messages stay on one line. Pane changes do not crash the long header;
  it clips safely around GPUI 0.2's mutable UTF-8 ellipsis-run bug.

The controlled scenarios are part of the normal QA peer (`--fixture`), not live
provider inferences; they execute no commands and edit no files. Screenshots and
QA stores remain outside the repository.

## Component spacing and diff-review follow-up

Compared the installed reference application work log, reasoning picker, and stacked/split diff
panel at matching native sizes. Shared activity rows now reserve a 24 px icon
column and a 16 px trailing disclosure slot with 12 px chevrons. Expanded content
uses the same 28 px inset. Consecutive work rows have no extra inter-row gap;
turn boundaries and prose transitions have separate spacing roles.

Command output is one bounded, independently scrolling inset pane with a copy
header, rather than dozens of virtual-history rows. The full output remains
available for copying and existing preview limits remain enforced. Composer
picker rows, search, selected backgrounds, and check marks fill their menu width;
reasoning labels use readable casing without changing backend routing values.

The changes panel now stacks all files with collapsible path/count headers,
syntax colors, old/new gutters, replacement pairing in split mode, wrapping,
fixed-gutter horizontal navigation, and an optional file navigator. It measures
the actual viewport: a requested 560 px inspector can shrink to about 404 px in
a 1100 px window without clipping a column or wrapping against the wrong width.
Stable diff content is virtualized; the main transcript retains its existing
virtual history and streaming hot-tail architecture.

Native checks used 1305×964 and 1100×800 windows:

- Real Codex created two disposable Rust/JSON files after Allow once. Both stayed
  absent before approval; the completed review showed the exact eight added lines.
  Reopening retained the reply and Applied review, using the valid local login.
- Expanded the real `printf` command to read `alpha` and `beta` in its output pane.
  Its chevron, code preview, and following response shared the revised alignment.
- Selected reasoning effort at the far right of a full-width menu row. Model
  search displayed and recovered from No matches; permission choices fit the
  narrow composer and menus dismissed correctly.
- Controlled `ui-thinking` displayed a single reasoning delta immediately,
  expanded at the shared inset, wrapped within a 400 px chat pane, and stopped.
- Controlled `ui-diff` exercised replacement, added/deleted files, a long code
  line, and 40 additional configuration lines. Checked stacked/split modes,
  wrapping, horizontal endpoint access, file navigation, collapse-all, and
  individual deleted-file disclosure. This peer emits protocol observations;
  it does not write those fixture files to disk.

The horizontal track is inset from the native window resize edge so its hit area
remains usable. Screenshots and QA runtime data remain outside the repository.

## Verification commands

- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets` — passed with repository warnings.
- `cargo check -p app` — passed with repository warnings.
- `cargo build -p app` — passed; the normal build was exercised natively.
- `cargo test --workspace` — 239 passed, 2 opt-in live tests ignored.
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
