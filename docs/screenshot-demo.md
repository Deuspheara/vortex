# Screenshot demo

The README screenshot is a native window capture of the app using isolated sample
data. It contains no personal sessions, credentials, or live model output.

From the repository root:

```sh
cargo build -p app --features demo_mode
cargo test -p app --features demo_mode fixture_is_isolated_idempotent_and_replayable -- --nocapture
python3 scripts/run-screenshot-demo.py --window-size 1600x1000
```

The script creates a disposable directory and `.app` bundle, prints both paths, strips provider credentials, and supplies Ghostty dylib paths. It does not change HOME. The Debug-only demo requires an absolute VORTEX_DEMO_DATA_DIR containing `.screenshot-demo`; database, project files and theme exports use this directory. Close the window to exit. Normal builds do not seed this data. Release compilation was not run.

The real SQLite event store receives nine deterministic AgentEvents for Atlas Notes. The actual reducer, timeline, tool row and diff viewer render them. No model, tool execution, patch application, test result or usage measurement is claimed. The initial and final assistant text share a stream; Activity mode is selected for demo builds to retain that stream before trailing plan/diff items. Production default remains unchanged.

Capture: the Atlas Notes session and proposed diff open at startup. The launcher
defaults to 1600×1000 logical pixels (clamped to 90% of the display); use
`--window-size WIDTHxHEIGHT` to override it. Capture only the native app window,
without other desktop content, and save it to `docs/assets/vortex.jpg`. The
README capture was refreshed on 16 October 2026 using this workflow and Codex
computer-use `getScreenshot`. Demo builds hide live-provider setup guidance in the thread and title bar,
while preserving the actual readiness state. Normal builds retain that guidance.
The composer model selector is not evidence that that model ran.

Capture scope: the screenshot verifies the thread and proposed diff presentation.
It does not validate every navigation action. Earlier demo testing encountered
unreliable Plan/inspection navigation and a GPUI `RefCell already borrowed`
abort; this capture does not establish that those issues are resolved. The fixture
test validates isolation, idempotency, and event storage, rather than GPUI
interaction behavior.
