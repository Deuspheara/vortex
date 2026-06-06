# Security Policy

Vortex is an early-stage developer preview. Please report suspected
vulnerabilities privately before opening public issues.

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
- browser and shell sidecar boundaries

## Secret policy

Vortex must not require checked-in credentials. Keep API keys and provider
tokens in the shell environment or OS-level secret storage. Local runtime state
belongs outside the repository, normally under `~/.config/vortex/`.
