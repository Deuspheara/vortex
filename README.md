# Vortex

Vortex is an experimental desktop agent UI built in Rust with GPUI and
`gpui-component`. It is designed around transparent local agent workflows:
project indexing, event-sourced sessions, approval-gated tool execution, and a
visible timeline of model and tool activity.

> Status: early-stage developer preview. The code is suitable to publish and
> inspect, but the app is not yet a hardened end-user product. Treat
> production readiness as an explicit checklist, not as the default state of
> this branch.

## What Vortex does

- Runs as a native desktop app on macOS.
- Uses OpenRouter for real model calls when `OPENROUTER_API_KEY` is set.
- Falls back to a mock provider when no API key is configured, so the UI can be
  explored without credentials.
- Stores local sessions in SQLite under `~/.config/vortex/`.
- Indexes local projects for search and context assembly.
- Routes tool execution through approval, sandbox, and patch-first policies.
- Runs virtual bash through a Rust-native fake shell over the workspace.
- Can route browser snapshot and screenshot tools through a user-configured MCP
  stdio server.

## Safety model

Vortex is built to keep agent activity inspectable:

- Provider streams are normalized into `AgentEvent` before UI rendering.
- Writes are represented as previewable patches before apply.
- Dangerous commands require approval.
- Runtime sessions are replayable from SQLite event data.
- Secrets are read from environment variables only and should never be
  committed.

The deeper runtime contracts live in:

- [Context and tool contract](docs/context-and-tool-contract.md)
- [Edit transaction contract](docs/edit-transaction-contract.md)
- [Agent guide](AGENTS.md)

## Requirements

- macOS, the primary development target.
- Rust stable with Edition 2024 support. The repository pins `stable` plus
  `rustfmt` and `clippy` in [rust-toolchain.toml](rust-toolchain.toml).
- Xcode command line tools for native macOS linking.

## Setup

From a fresh clone, install the pinned Rust toolchain components and verify the
desktop app:

```bash
rustup show
cargo check -p app
```

No `.env` file is required or loaded. For real model calls, export the key in
your shell before starting the app:

```bash
export OPENROUTER_API_KEY=...
cargo run -p app
```

Without `OPENROUTER_API_KEY`, Vortex starts with the mock provider.

## Running

Run from the repository root:

```bash
cargo run -p app
```

The app writes local runtime state outside the repository. To reset local app
state during development, stop the app and move or delete `~/.config/vortex/`.
Do not commit local databases, logs, traces, or generated build output.

Useful development commands:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo check -p app
cargo test --workspace
```

## Optional configuration

| Variable | Purpose |
| --- | --- |
| `OPENROUTER_API_KEY` | Enables real OpenRouter model calls. If omitted, the mock provider is used. |
| `PAGEINDEX_API_KEY` | Optional key for hosted document/PDF context provider experiments. |
| `EXA_API_KEY` | Optional key for Exa-backed web search tools. |
| `TAVILY_API_KEY` | Optional key for Tavily-backed web search tools. |
| `JINA_API_KEY` | Optional key for Jina-backed URL extraction. |
| `FIRECRAWL_API_KEY` | Optional key for Firecrawl-backed URL extraction. |
| `VORTEX_CONTEXT_SELECTION` | Set to `1`, `true`, or `yes` to enable context-selection prompt behavior. |
| `VORTEX_RENDER_PROFILE` | Set to `1` or `true` to enable render profiling instrumentation. |
| `VORTEX_BROWSER_MCP_COMMAND` | Optional command used to start a browser MCP stdio server. If omitted, browser tools are disabled with a clear tool denial. |
| `VORTEX_BROWSER_MCP_ARGS` | Optional JSON array of string arguments for `VORTEX_BROWSER_MCP_COMMAND`. |
| `VORTEX_BROWSER_MCP_SNAPSHOT_TOOL` | MCP tool name that backs Vortex's `browser_snapshot` tool. Required when `VORTEX_BROWSER_MCP_COMMAND` is set. |
| `VORTEX_BROWSER_MCP_SCREENSHOT_TOOL` | MCP tool name that backs Vortex's `browser_screenshot` tool. Required when `VORTEX_BROWSER_MCP_COMMAND` is set. |

Runtime data is stored outside the repository in `~/.config/vortex/`. If the
home directory cannot be resolved, the app falls back to `.vortex/`, which is
ignored by Git.

Example browser MCP configuration:

```bash
export VORTEX_BROWSER_MCP_COMMAND=...
export VORTEX_BROWSER_MCP_ARGS='["arg-one","arg-two"]'
export VORTEX_BROWSER_MCP_SNAPSHOT_TOOL=...
export VORTEX_BROWSER_MCP_SCREENSHOT_TOOL=...
cargo run -p app
```

## Repository layout

```text
crates/
  agent_protocol/    Shared command, event, tool, risk, and status types
  agent_store/       SQLite event log and session/project persistence
  agent_core/        Runtime state machine and model loop
  agent_models/      Mock and OpenRouter providers
  agent_tools/       Tool registry and tool executors
  agent_sandbox/     Risk, approval, path, and secret policies
  agent_context/     Prompt and context builder
  agent_shell/       Rust-native virtual shell
  real_process/      Approved real command executor
  project_index/     File walk, search, summaries, and project index cache
  context_providers/ Context provider service and adapters
  terminal/          Terminal core and renderer
  android_device/    Android observation and action helpers
  app/               GPUI desktop shell
docs/                Architecture, runtime, editing, release, and performance notes
```

## Production readiness

Vortex is designed around safety boundaries, but a production release should be
cut only after the release owner verifies the current branch against the full
checklist in [Production readiness](docs/production-readiness.md).

At minimum, do not publish a production build until:

1. CI is green on the release commit.
2. `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`,
   `cargo check -p app`, and `cargo test --workspace` pass locally or in CI.
3. A secret scan covers tracked files and git history.
4. Runtime data is confirmed outside the repository.
5. README, security policy, and user-facing setup instructions match the
   release behavior.

## Public repo hygiene

Before pushing a public repository:

1. Run `git status --short --branch` and confirm only intentional files are
   changed.
2. Run a secret scan over tracked files and history, for example with
   `gitleaks detect --source .` or an equivalent scanner.
3. Keep credentials in your shell or OS keychain, not in `.env` or committed
   config.
4. Confirm generated outputs such as `target/`, `node_modules/`, logs, traces,
   and local SQLite files are ignored.
5. Confirm `Cargo.lock` is tracked and up to date.

This repository includes ignore rules for common local artifacts and a
[security policy](SECURITY.md) for private vulnerability reports.

## License

Vortex is open source under the [MIT License](LICENSE).
