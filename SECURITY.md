# Security Policy

Vortex is an early-stage developer preview. Please report suspected
vulnerabilities privately before opening public issues. Do not rely on the app
as a hardened sandbox for hostile repositories, hostile prompts, or untrusted
MCP servers.

## Reporting a vulnerability

If you find a security issue, contact the repository owner privately through
GitHub. Include:

- affected commit or version
- impact and affected component
- reproduction steps or proof of concept
- whether credentials, local files, or command execution are involved

Do not include real secrets in reports.

## Scope

Security-sensitive areas include:

- provider credentials and environment variable handling
- sandbox policy and approval bypasses
- patch preview/apply behavior
- real command execution
- project indexing of ignored, hidden, binary, or large files
- local SQLite session storage
- browser MCP server configuration and tool routing
- Rust-native virtual shell behavior
- cancellation of runs, tools, and real processes

## Secret policy

Vortex must not require checked-in credentials. Keep API keys and provider
tokens in the shell environment or OS-level secret storage. Local runtime state
belongs outside the repository, normally under `~/.config/vortex/`.

## Operational guidance

- Run with the mock provider when validating UI behavior without credentials.
- Keep the default safety mode at `ApplyWithApproval` unless a specific test
  requires another mode.
- Review every patch proposal before applying it.
- Configure browser MCP servers only from trusted local commands.
- Remove or rotate any credential that was exposed in a prompt, log, trace,
  SQLite database, or git history.
