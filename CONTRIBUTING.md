# Contributing

Vortex is an early-stage Rust desktop app. Contributions should keep the
runtime safety model, UI consistency, and public repo hygiene intact.

## Development setup

```bash
cargo check -p app
```

Run commands from the repository root. The workspace is Rust-first; no
JavaScript sidecar install is required for the current app.

## Before opening a pull request

Run the checks that apply to your change:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo check -p app
cargo test --workspace
```

If a check cannot run on your machine, mention that in the PR.

The GitHub CI workflow runs these same checks on macOS.

## Code expectations

- Keep changes scoped and reviewable.
- Follow [AGENTS.md](AGENTS.md) for architecture, UI, and runtime rules.
- Keep UI spacing, colors, row heights, and radii token-driven.
- Keep components mostly stateless; put app mutation in window orchestration.
- Add tests when changing runtime behavior, tool contracts, parsing, sandboxing,
  persistence, or other shared logic.
- Update user-facing docs when setup, configuration, safety behavior, or release
  expectations change.

## Pull request checklist

Before opening a PR:

1. Confirm `git status --short --branch` contains only intentional changes.
2. Run the narrowest relevant test while developing.
3. Run the full check set before review when the change is ready.
4. Document skipped checks and known limitations in the PR body.
5. For UI changes, include the manual path exercised and follow the flat-row and
   token rules in [AGENTS.md](AGENTS.md).

## Secret handling

- Do not commit `.env` files, API keys, provider tokens, local databases,
  terminal logs, trace files, or generated build outputs.
- Use environment variables for local provider credentials.
- Run a secret scan before publishing a branch from a private workspace.
- Treat MCP server commands and arguments as local configuration, not repository
  defaults that should embed user-specific paths or credentials.
