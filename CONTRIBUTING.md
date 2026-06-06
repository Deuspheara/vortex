# Contributing

Vortex is an early-stage Rust desktop app. Contributions should keep the
runtime safety model, UI consistency, and public repo hygiene intact.

## Development setup

```bash
cargo check -p app
```

Install sidecar dependencies only when working on the TypeScript sidecars:

```bash
cd sidecars/just_bash_host
bun install

cd ../browser_worker
bun install
```

## Before opening a pull request

Run the checks that apply to your change:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo check -p app
cargo test --workspace
```

If a check cannot run on your machine, mention that in the PR.

## Code expectations

- Keep changes scoped and reviewable.
- Follow [AGENTS.md](AGENTS.md) for architecture, UI, and runtime rules.
- Keep UI spacing, colors, row heights, and radii token-driven.
- Keep components mostly stateless; put app mutation in window orchestration.
- Add tests when changing runtime behavior, tool contracts, parsing, sandboxing,
  persistence, or other shared logic.

## Secret handling

- Do not commit `.env` files, API keys, provider tokens, local databases,
  terminal logs, trace files, or generated build outputs.
- Use environment variables for local provider credentials.
- Run a secret scan before publishing a branch from a private workspace.
