# Vortex docs

This folder contains durable architecture, runtime, and release notes. Keep
short-lived handoff notes out of this folder unless they are converted into
maintained guidance.

## Start here

- [Production readiness](production-readiness.md): release gate, smoke tests,
  configuration audit, and documentation checklist.
- [Context and tool contract](context-and-tool-contract.md): how Vortex keeps
  agent context small, tool results useful, and model actions inspectable.
- [Edit transaction contract](edit-transaction-contract.md): patch-first edit
  model, validation flow, and quality gates.

## UI and performance

- [Thread performance](thread-performance.md): current streaming thread
  architecture, profiling points, anti-patterns, and follow-up targets.
- [Rendering performance audit](render-performance-audit.md): debug profiling
  switch and known rendering hot paths.
- [Right panel tabs](right-panel-tabs.md): inspector tab model and extension
  rules.

## Maintenance rules

- Prefer docs that describe current contracts over one-off task briefs.
- Update links when code moves between `features/`, `shared/`, `window/`, and
  compatibility modules.
- Put release-facing checklists in [Production readiness](production-readiness.md)
  rather than duplicating them across docs.
- Keep repo-specific agent rules in [AGENTS.md](../AGENTS.md).
