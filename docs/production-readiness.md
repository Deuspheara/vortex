# Production readiness

This checklist is the release gate for Vortex. Vortex is still a developer
preview, so "production ready" means the current branch has been deliberately
validated for public use; it does not mean the app has completed a full
security hardening program.

## Release decision

Before cutting a public release, assign one release owner and record:

- release commit SHA
- target audience
- supported platform
- known limitations
- verification commands and results
- unresolved security or data-loss risks

Do not release from a dirty working tree.

## Required checks

Run these from the repository root:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo check -p app
cargo test --workspace
```

The CI workflow runs the same checks on macOS. If a check is skipped, document
the reason in the release notes or pull request.

## Runtime safety

Confirm the default safety posture before release:

- Default agent mode is `ApplyWithApproval`.
- Real command execution requires approval.
- File writes are represented as patches before apply.
- Dangerous command patterns require approval.
- Browser MCP tools are disabled when no MCP command is configured.
- Provider events are normalized into `AgentEvent` before UI rendering.
- Cancellation works for active runs and pending tools.

Relevant contracts:

- [Context and tool contract](context-and-tool-contract.md)
- [Edit transaction contract](edit-transaction-contract.md)
- [Agent guide](../AGENTS.md)

## Secrets and data

Vortex must not require checked-in credentials.

- Keep provider keys in environment variables or OS-level secret storage.
- Do not commit `.env`, `.env.*`, key files, local databases, logs, traces, or
  generated outputs.
- Run a secret scan over tracked files and git history before publishing.
- Confirm local sessions are stored under `~/.config/vortex/`.
- Confirm the `.vortex/` fallback directory is ignored by Git.

Useful command:

```bash
git status --short --branch
```

## Configuration audit

Document which optional integrations are enabled for the release:

| Area | Variables |
| --- | --- |
| Model provider | `OPENROUTER_API_KEY` |
| Context providers | `PAGEINDEX_API_KEY` |
| Web tools | `EXA_API_KEY`, `TAVILY_API_KEY`, `JINA_API_KEY`, `FIRECRAWL_API_KEY` |
| Browser MCP | `VORTEX_BROWSER_MCP_COMMAND`, `VORTEX_BROWSER_MCP_ARGS`, `VORTEX_BROWSER_MCP_SNAPSHOT_TOOL`, `VORTEX_BROWSER_MCP_SCREENSHOT_TOOL` |
| Diagnostics | `VORTEX_CONTEXT_SELECTION`, `VORTEX_RENDER_PROFILE` |

For any configured MCP server, verify the configured tool names are advertised
by the server and that tool failures are shown as clear denials or errors in
the app.

## Manual smoke test

For a release candidate, run the app with no provider key and verify:

1. The app starts with the mock provider.
2. A new conversation can be created.
3. Project indexing does not include ignored build output.
4. A read-only tool result renders in the thread.
5. A patch proposal is previewable before apply.
6. A dangerous or real command pauses for approval.
7. Rejecting an approval leaves no unknown file changes.
8. Cancelling a run stops active work and clears pending UI state.

Then run with `OPENROUTER_API_KEY` in the shell environment and verify one real
model call completes without logging the key.

## Documentation gate

Before publishing, update these files when behavior has changed:

- [README.md](../README.md): setup, run commands, configuration, limitations
- [CONTRIBUTING.md](../CONTRIBUTING.md): contributor checks and coding rules
- [SECURITY.md](../SECURITY.md): reporting path, scope, secret handling
- [AGENTS.md](../AGENTS.md): architecture and agent rules

Public documentation should state current limitations directly. Do not imply
that the app is hardened for untrusted repositories, untrusted MCP servers, or
unreviewed tool execution unless those claims have been tested.
