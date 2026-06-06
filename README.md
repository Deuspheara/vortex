# Vortex

Vortex is an experimental desktop agent UI built in Rust with GPUI and
`gpui-component`. It is designed around transparent local agent workflows:
project indexing, event-sourced sessions, approval-gated tool execution, and a
visible timeline of model and tool activity.

> Status: early-stage developer preview. The code is suitable to publish and
> inspect, but the app is not yet a hardened end-user product.

## What Vortex does

- Runs as a native desktop app on macOS.
- Uses OpenRouter for real model calls when `OPENROUTER_API_KEY` is set.
- Falls back to a mock provider when no API key is configured, so the UI can be
  explored without credentials.
- Stores local sessions in SQLite under `~/.config/vortex/`.
- Indexes local projects for search and context assembly.
- Routes tool execution through approval, sandbox, and patch-first policies.
- Keeps JavaScript-specific sidecars isolated under `sidecars/`.

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
- Bun for the TypeScript sidecars.

## Setup

Install Rust dependencies and verify the desktop app:

```bash
cargo check -p app
```

Install sidecar dependencies:

```bash
cd sidecars/just_bash_host
bun install

cd ../browser_worker
bun install
```

No `.env` file is required or loaded. For real model calls, export the key in
your shell before starting the app:

```bash
export OPENROUTER_API_KEY=...
cargo run -p app
```

Without `OPENROUTER_API_KEY`, Vortex starts with the mock provider.

## Running

Run from the repository root so the app can locate the sidecars:

```bash
cargo run -p app
```

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
| `VORTEX_CONTEXT_SELECTION` | Set to `1`, `true`, or `yes` to enable context-selection prompt behavior. |
| `VORTEX_RENDER_PROFILE` | Set to `1` or `true` to enable render profiling instrumentation. |

Runtime data is stored outside the repository in `~/.config/vortex/`. If the
home directory cannot be resolved, the app falls back to `.vortex/`, which is
ignored by Git.

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
sidecars/
  just_bash_host/    Bun JSON-RPC sidecar for virtual bash execution
  browser_worker/    Bun/Playwright sidecar for browser inspection
docs/                Runtime, editing, and performance notes
```

## Public repo hygiene

Before pushing a public repository:

1. Run `git status --short --branch` and confirm only intentional files are
   changed.
2. Run a secret scan over tracked files and history.
3. Keep credentials in your shell or OS keychain, not in `.env` or committed
   config.
4. Confirm generated outputs such as `target/`, `node_modules/`, logs, traces,
   and local SQLite files are ignored.

This repository includes ignore rules for common local artifacts and a
[security policy](SECURITY.md) for private vulnerability reports.

## License

Vortex is open source under the [MIT License](LICENSE).
