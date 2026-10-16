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

Command output remains one bounded, independently scrolling virtual-history row.
The later whole-thread pass below replaces its inset pane/copy header with flat
text and a hover copy action. The full output remains
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

## Sidebar refinement

Removed the sidebar's extra right border; the resizable layout now owns the
single sidebar/thread separator. The search field has a compact input surface,
a clear control, and explicit focus across its hit area. Project rows remain
flat when expanded, while only the selected conversation receives a selection
background. Shared conversation rows use a semantic chat glyph and aligned text;
section actions have tooltips and the footer has less visual separation.

Native checks verified search filtering and clearing, project collapse/expand,
conversation selection, and widening the sidebar through the remaining resize
handle. The revised sidebar was inspected at 1305×964 and 1100×800. The screenshot
and isolated QA data remain outside the repository.

## Delete confirmation and sidebar scrolling

Conversation/project deletion previously used blocking `rfd::MessageDialog::show`
inside a deferred AgentWindow update. On macOS the alert enters a nested Cocoa
loop while GPUI's AppCell is still borrowed; frame tasks can then repeatedly fail
with `RefCell already borrowed` and abort. Both confirmations now use GPUI's
non-blocking native prompt receiver. The async task awaits the user's decision
before reacquiring the view; Cancel and dismissed prompts perform no deletion.

Removed sidebar row entrance, expansion, title, metadata, container fade, and
drag-divider animations. Virtualized rows now render directly, so recycling rows
while scrolling does not restart a cascade or temporarily shift/fade the text.

Native verification used disposable QA data: cancelled and confirmed chat deletion,
verified automatic selection of the remaining chat, cancelled and confirmed project
removal, and verified recovery into a usable new workspace conversation. A 25-chat
project exercised Show more and scrolling in both directions. Deleting a background
chat preserved the selected chat; reopening retained the resulting 24-chat count.
Escape cancelled a new confirmation on the rebuilt app after reopening. No panic,
borrow error, or error-level entry appeared in either QA log. Real user data was
untouched.

## Context-meter accounting correction

The ring previously accumulated processed input across model calls, divided by
a catalog fallback of 200K even for Codex. The todo task's saved Codex usage proves
102,469 processed input tokens across five requests versus 22,098 current-context
tokens and a reported 475K capacity. Current occupancy now has its own normalized,
persisted snapshot; run input/output/cache totals remain separate. The tooltip
labels run totals explicitly, and fresh-context advice uses occupancy instead of
cumulative processing. Older Codex runs without saved snapshots show unavailable
context instead of an invented percentage.

Regression coverage includes latest-request versus cumulative/cache accounting,
capacity changes, compaction decreasing occupancy, repeated snapshots after
projection serialization, missing usage/capacity, native latest-request accounting,
and store persistence of all five controlled protocol updates. The normal native
QA app replays the exact todo measurements through a deterministic peer; this
performs no inference or file changes. Native tooltip verification showed
22,098 / 475K alongside a separately labelled 102.5K run-input total. Reopening
retained the snapshot with exactly one user submission. Switching to an older
conversation showed unavailable usage rather than inheriting the other chat’s
value. See the context-accounting section in
[codex-backend.md](codex-backend.md) for the investigation and its limits.

## Thread spacing and failure presentation follow-up

Compared the installed reference application window (currently its empty workspace) and the
available reference application timeline/WorkLog source. Conversational turn and first-activity gaps
are now 16 px, with 8 px between activity and prose; consecutive activity headers
retain their uninterrupted 24 px rhythm. This removes the previous 32/24 px gaps
without changing virtualized history or the streaming hot tail.

Failures use a flat circle-alert row with a destructive-colored title. Title,
details, and actions share one text column and an explicit 14 px font with 20 px
line height. The title-to-detail gap is 4 px; actions follow the details by 8 px.
Open settings uses a compact bordered, filled button with an explicitly styled
label, avoiding the component library’s larger default label typography. Action
IDs include the error item identity. Removed the amber stripe and empty action
spacing. Cancellation uses a neutral stopped row. Errors now participate in cached width-aware layout measurement,
including wrapped details and settings actions, so their estimated height cannot
leave blank space or overlap the following turn after resizing.

Exercised the normal native app using the deterministic Codex protocol peer:
unsupported-request failure, wrapped details in the narrow chat lane alongside
Changes, Open settings and return, streaming reasoning/code, Stop, and a completed
command/code response. Reopened at 1305×964 and toggled Changes to verify error
remeasurement from narrow to full-width lanes, with details and actions remaining
visible. Runtime logs contained no errors or borrow panics. No live inference was
needed for these presentation checks.

## Whole-thread typography and spacing comparison

Opened the installed reference application, including populated conversations under
Settled. Created a separate “Compare Command and Code Rendering” conversation
without a project, selected Supervised execution, and approved only the requested
`printf 'alpha\nbeta\n'` command. Expanded its work group, command output, and
reasoning, and inspected the Rust preview. No project files were inspected or
changed. Appearance settings confirmed SF Pro, interface base 16 px, Menlo code
13 px, Comfortable width. The reference application’s thread text uses the 14 px role within that base.

The corresponding native Vortex changes cover the whole transcript:

- 14 px prose/activity text, 20/18/16 px headings with explicit line heights,
  13 px Menlo code/output, and 11 px code-language labels. Prose uses the reference
  foreground tone; headings remain stronger. Text buttons use the small type role.
- Consecutive thinking and command rows keep a 24 px baseline rhythm and the same
  24 px icon column / 6 px label gap. Collapsed reasoning shows its summary;
  expanded reasoning uses Thought/Thinking and indented details. Commands show
  their command text without a redundant Ran/Running prefix.
- Expanded command/output content is flat and bounded. Removed the Output header,
  bordered panel, and fixed label columns. Wrapped output participates in actual
  width measurement, with a 256 px total height cap and independent scrolling.
- Paragraph/list blocks use 12 px spacing, headings get 20 px lead-in and 8 px
  follow-on spacing (12 px before code). This replaces the uniform 16 px block gap.
- Completed and open streaming code fences share one preview renderer: 13 px code
  on an 18 px line grid, a compact 30 px toolbar, balanced body padding, and copy
  and wrapping controls. Wrap changes invalidate measured heights. The control
  index survives the background parse transition; session switches clear local
  preview preferences. Stable history remains virtualized and streaming retains
  its hot-tail path.

Native verification for the whole-thread pass used the normal application with
an isolated store and deterministic app-server peer at 1305×964 and 1100×800.
Checked a clean conversation with paragraphs, lists, two heading levels, collapsed
and expanded reasoning/commands, flat output, long wrapped code, both wrap states,
and following-message placement. Reopened the conversation, toggled the adjacent
Changes panel to force remeasurement, exercised an open streaming fence and Stop,
and rejected an approval back to idle. The 247-test workspace run includes a new
regression for stable code-control identity across background prefix parsing;
the existing output-height cap regression also passes. Runtime logs were clean.

## Verification commands

- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets` — passed with repository warnings.
- `cargo check -p app` — passed with repository warnings.
- `cargo build -p app` — passed; the normal build was exercised natively.
- `cargo test --workspace` — 247 passed, 2 opt-in live tests ignored.
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

## Long approvals and opt-in AI review — 2026-10-16

Normal debug application, not demo mode, tested with Codex CLI 0.163.0.
Controlled protocol scenarios used an isolated store/workspace; the real-account
check used a second fresh disposable workspace and the existing local login.
No credentials or generated screenshots/databases/logs are committed.

- At 1100×800 and 900×600, a 123-line Python heredoc remained fully scrollable
  through `# COMMAND-END` and its closing delimiter. Allow once and Deny stayed
  visible and were exercised; the composer draft is retained behind a compact
  review/Stop run footer. The fixture never executes the displayed command.
- The Changes-panel preview uses the same bounded request widget and full-text
  copy action. At 900×600 both approval footers remained reachable. Narrow review
  layouts temporarily hide the sidebar and cap the adjacent panel width; closing
  Changes restores the sidebar. Reject from Changes and Stop run both worked.
- Content clicks previously could begin Cocoa's transparent-titlebar dragging
  behavior. Preventing that default below the titlebar restored reliable native
  control/input clicks; titlebar dragging remains available.
- The Codex permission picker exposes AI review alongside Plan and Approval
  required. Settings explains the restricted sandbox and extra model usage.
  Native backend behavior is unchanged; Codex's default remains Approval required.
- Controlled native runs exercised Reviewing, Approved, Denied, Timed out, and
  cancellation. Expanded review activity disclosed risk, authorization, and
  rationale. Stopping an unfinished review showed “no approval decision was
  confirmed.” These outcomes survived quitting/reopening without a new turn.
- A real native conversation using `gpt-6.1-sol`, Low effort, and AI review asked
  for one harmless local write. The installed Codex reviewer approved the exact
  command and reported low risk/high authorization with a rationale. Vortex
  displayed the review separately from command execution. Independent file
  inspection confirmed the disposable workspace contained only
  `vortex-native-review.txt` with the exact bytes `after\n`. No manual approval
  response was sent. A separate opt-in backend test also exercised a real review.
- Review event normalization deduplicates completions, persists its cursor,
  handles completed-only notifications, and fails unknown statuses visibly.
  Unrelated review/tool completions cannot dismiss a pending manual approval.

Verification: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`,
`cargo check -p app`, `cargo build -p app`, `cargo test --workspace` (253 passed,
3 opt-in account tests ignored), and
`cargo test -p app --features demo_mode fixture_is_isolated_idempotent_and_replayable`
all passed. Existing workspace warnings remain. The opt-in
`live_codex_automatic_review_keeps_restricted_execution` test was additionally
run and passed against the installed account.

Limits: AI review depends on the installed Codex version/account and uses extra
model calls. It is a risk assessment, not a safety guarantee. Denial, timeout,
and interrupted review were exercised with deterministic protocol fixtures;
only successful approval was induced against the real account. Unsupported
interactions still fail closed. Codex-owned changes retain the limitations
explained in [Codex backend](codex-backend.md).

## Quiet command presentation — 2026-10-16

Supersedes the visible review-report treatment above. Reopened the saved real
Codex conversation used for the previous verification in the normal native app
at 1100×800. Its command now displays `printf 'after\n' > vortex-native-review.txt`
without the `/bin/zsh -lc` launcher. There is one command row, no duplicate body,
no expansion arrow for empty output, and no successful-review report.

At 900×600, the deterministic native fixture verified:

- A real output disclosure expands to `alpha` and `beta` under a single command
  header, aligned on a subtle rail, without repeating the command.
- A long Python heredoc has a first-line header and a 123-line count; its full
  contents remain scrollable through the final delimiter in a bounded detail area.
- A denied automatic review shows “Command not approved” without risk,
  authorization, rationale, or an empty disclosure arrow. Routine review activity
  is hidden in Focus and Activity modes. Debug mode retains the persisted audit.
- Manual approval cards retain the exact request and actions, without decorative
  risk labels. Execution, sandbox policy, and approval routing are unchanged.

Shell unwrapping is presentation-only and limited to an exact three-argument
`sh`/`bash`/`zsh` wrapper with a recognized command flag. Extra arguments, other
interpreters, and malformed quoting remain intact. Tests cover quoted inner
commands, empty/duplicate output, disclosure rows, and review visibility.

Formatting, workspace Clippy, app check/build, workspace tests (256 passed,
3 account tests ignored), and the demo fixture test passed. Existing warnings
remain. No additional real inference was needed: the real conversation was
replayed from its existing event store, and new rejection/output scenarios used
the nonexecuting protocol fixture.

## Thread hierarchy, live previews, and scroll stability — 2026-10-16

Normal native app checked at 1100×800 and 900×600, including reopening the saved
real-account conversation and running new real Codex file edits. This supersedes
the previous flat expanded-command treatment: the user explicitly requested a
small Shell card for expanded command contents.

- Assistant steps, summaries, and bullet text use the full theme foreground
  (black in light themes, readable foreground in dark themes). Commands and their
  completed group use the secondary foreground. Successful commands fold by
  user turn after execution finishes. The group expands/collapses independently
  of each command. Failures and approval prompts remain visible.
- Expanded command/script details use a bounded Shell card with the readable
  command, output, and copy action. Command execution and persisted events retain
  their exact original contents. Terminal, search, check, and disclosure icons
  use the existing gpui-component icon library.
- Bullet gaps use the tight spacing token and the same 23 px line height as
  paragraphs. Inline code reserves whitespace on each side and paints rounded
  backgrounds from the actual shaped text positions, retaining natural wrapping.
  Unicode/highlight-boundary tests cover padded code nested inside bold text.
- Sidebar search has one padded field, a library search icon, a clear action,
  and “Search chats” placeholder. Native filtering and clearing were exercised.
- Codex turn-level diff notifications update the Changes panel while the turn
  runs. The protocol fixture changed the same preview at checkpoints 2, 15, 40,
  and 65 during streaming. Snapshots are deduplicated, persisted, and represented
  by one change summary per run instead of repeated summaries.
- A real native Codex conversation created `vortex-live-preview.txt` with
  `first\n`, then changed it to `second\n` in a separate apply_patch call. Both
  approval previews updated correctly, each edit was manually approved through
  the native UI, and the final observed diff showed `second`. Independent file
  inspection confirmed `second\n`; the previous QA file remained `after\n`.
  Reopening the app retained the final changes and one summary for the run.
- Codex readiness checks now cover its provider, project, and trust. Native
  indexing/context refresh no longer inserts a transient setup banner while
  Codex writes, which previously shifted the thread during real file generation.
- Scroll effects respect manual scrolling rather than resetting follow mode.
  Jump to latest is an overlay instead of changing the streaming text height.
  Exact streaming-row measurement happens before virtual-list row placement;
  cheap height estimates cannot repeatedly shrink measured content. This fixes
  alternating heights and overlaps with activity following the streaming reply.
  Overscan measurements and completion regrouping preserve a stable row anchor.
  At 900×600, checkpoint 14 remained at the same vertical position before and
  after completion while the adjacent preview advanced from checkpoint 15 to 65;
  Jump to latest returned to the final response without inserting a text row.

Regression coverage includes command grouping with failures left visible,
completion scroll anchoring, Unicode inline code, native readiness isolation,
and live diff updates/deduplication/replay. Formatting, workspace Clippy, app
check/build, workspace tests (262 passed; 3 opt-in account tests ignored), and the
demo fixture test passed. Existing warnings remain.

Limits: live file previews update when Codex reports a file-change or turn-diff
snapshot. This does not invent keystroke-level file contents or guarantee that
Codex reports every shell-owned write. Streaming/scroll checks used controlled
protocol generation; the real-account native run verified file approvals,
preview updates, final contents, and replay.
