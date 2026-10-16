# Portfolio demo — 15 October 2026

Validated commands (repository root):

```sh
cargo build -p app --features demo_mode
cargo test -p app --features demo_mode fixture_is_isolated_idempotent_and_replayable -- --nocapture
python3 scripts/run-portfolio-demo.py
```

The script creates a disposable directory and `.app` bundle, prints both paths, strips provider credentials, and supplies Ghostty dylib paths. It does not change HOME. The Debug-only demo requires an absolute VORTEX_DEMO_DATA_DIR containing `.portfolio-demo`; database, project files and theme exports use this directory. Close the window to exit. Normal builds do not seed this data. Release compilation was not run.

The real SQLite event store receives nine deterministic AgentEvents for Atlas Notes. The actual reducer, timeline, tool row and diff viewer render them. No model, tool execution, patch application, test result or usage measurement is claimed. The initial and final assistant text share a stream; Activity mode is selected for demo builds to retain that stream before trailing plan/diff items. Production default remains unchanged.

Capture: select the Atlas Notes session in the sidebar after startup; the real proposed diff is open. Native window capture via Codex computer-use `getScreenshot`, 1440×1000, English, dark. The offline provider setup banner and composer model selector are existing UI; the selector is not evidence that that model ran.

Limitations: only the review screenshot was retained. Repeated attempts to open Plan, expand inspection and close/reopen the diff did not reliably update the current UI; an earlier instance also aborted with GPUI `RefCell already borrowed`. The complete navigation acceptance is not validated. No invented alternate screens were made. The fixture test validates isolation/idempotency/event storage, not a GPUI reducer integration test. Existing unrelated staged UI changes were preserved.
