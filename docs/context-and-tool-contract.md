# Context and tool contract

The bottleneck for coding agents is usually not the model — it is the **tool contract**. Context must be managed like memory, not like an unbounded chat transcript.

Use this document as the product and architecture spec for Vortex agent tooling and context management.

---

## Bad vs good agent loops

**Bad (common default):**

```txt
task -> read many files -> put raw files in context -> think -> edit
```

**Good:**

```txt
task -> discover tiny relevant surface -> read only slices -> edit -> validate
```

---

## What popular tools do

**OpenCode** exposes separate tools for `glob`, `grep`, `read`, `lsp`, `edit`, `apply_patch`, and `todowrite`. Its `read` supports line ranges; `grep`/`glob` are first-class discovery tools so the agent does not load whole files just to find the right place. Its experimental LSP tool supports definitions, references, hover, document symbols, workspace symbols, implementations, and call hierarchy. ([OpenCode tools](https://opencode.ai/docs/tools/))

**Pi** is built around “context engineering”: project instructions via `AGENTS.md`, system prompt overrides via `SYSTEM.md`, compaction near the context limit, on-demand skills, prompt templates, and dynamic context through extensions that inject or filter messages before each turn. ([pi.dev](https://pi.dev/))

**Aider** uses a repository map: a compact representation of important files, classes, functions, signatures, and key lines. For large repos it selects only the most relevant parts via graph ranking instead of dumping the whole project. ([Aider repo map](https://aider.chat/docs/repomap.html))

**Claude Code** documents that the context window fills quickly because every message, file read, and command output stays in the session; performance degrades as it fills. It recommends aggressive context management and subagents for investigation. ([Claude Code best practices](https://code.claude.com/docs/en/best-practices))

Pi’s context-mode package describes another pattern: keep raw tool output out of the context window, store session state externally, retrieve only relevant events later, and have the model write small scripts for analysis instead of reading huge data into context. ([Pi context-mode](https://pi.dev/packages/context-mode))

---

## What to build

### 1. Make `read_file` a last-resort tool

Do not let the model read full files by default.

**Bad:**

```ts
read_file(path: string): string
```

**Good:**

```ts
read_file_slice({
  path: string,
  startLine: number,
  endLine: number,
  reason: string,
  maxBytes?: number
})
```

**Even better:**

```ts
inspect_file({
  path: string,
  goal: "find component API" | "find todo model" | "find route handler",
  budget: "tiny" | "normal" | "deep"
})
```

Returns symbol summaries and suggested slices; the model chooses only what it needs.

### 2. Add a repo map

For every project, index:

```txt
path, language, exports, classes, functions, types, imports,
routes, components, tests, last modified
```

Example for a todo app task:

```txt
Repo map:
- src/App.tsx: App component, uses TodoList, TodoInput
- src/components/TodoList.tsx: renders todos, toggle/delete callbacks
- src/components/TodoInput.tsx: input form, add callback
- src/store/todos.ts: Todo type, create/update/delete logic
- package.json: scripts: dev, test, build
```

Target: **500–1500 tokens** instead of **30k** of raw files.

### 3. Add search tools that return small snippets

```ts
list_files({ glob, limit })
search_code({ query, glob?, maxResults, contextLines })
find_symbol({ name, kind? })
references({ symbol, path? })
definition({ symbol, path? })
```

The model should search before reading.

**Good flow:**

```txt
User: add priority to todos
Agent:
1. list_files("src/**/*")
2. search_code("type Todo OR interface Todo")
3. read_file_slice("src/store/todos.ts", 1, 80)
4. search_code("TodoItem")
5. read_file_slice("src/components/TodoItem.tsx", 1, 120)
6. edit
```

**Bad flow:**

```txt
read src/App.tsx
read src/TodoList.tsx
read src/TodoItem.tsx
read src/store.ts
read package.json
read all tests
```

### 4. Create a “small task mode”

For simple tasks, enforce a strict policy:

```txt
Small task mode:
- Max 3 discovery calls.
- Max 2 file slices.
- No full-file reads.
- No architecture exploration unless required.
- Prefer direct patch.
- Summarize assumptions before editing.
```

**Triggers:** simple app, small UI fix, typo fix, TODO feature, single-file change.

For “make a simple todo app”:

```txt
1. Check package.json or existing structure.
2. Create/modify the minimal files.
3. Run build/test.
```

### 5. Store tool results outside the LLM context

Runtime keeps a **tool result store**:

```ts
ToolResult {
  id: string
  tool: string
  rawOutputPath: string
  summary: string
  extractedFacts: string[]
  relevantRanges: Range[]
  tokenCost: number
}
```

The model receives only compact summaries; raw output stays in SQLite / disk / memory.

### 6. Make tool outputs intentionally small

Defaults:

```ts
maxResults: 20
maxBytes: 8000
contextLines: 2
truncate: true
```

Every response should include when truncated and suggest `nextActions` (e.g. `read_file_slice`, narrower `search_code`).

### 7. Add a context budget manager

Before every model call:

```txt
system prompt: 1800 tokens
repo map: 1200 tokens
conversation summary: 900 tokens
active task state: 500 tokens
recent messages: 2500 tokens
tool results: 3000 tokens
free budget: 20000 tokens
```

When over budget: summarize old tool results, drop raw command logs, keep edited-file summaries and current plan.

**Preserve:** current task, files edited, decisions, failing errors, user constraints, next step.

**Drop:** full grep output, full build logs, duplicate file reads, old reasoning, irrelevant command output.

### 8. Inspection subagents with tiny output

Subagents return compact reports, not raw file dumps.

### 9. Cheap “analyzer script” tool

For bulk analysis, generate and run a script (or `rg`) and return only the result — not 80 full file reads.

---

## Minimal tool set (target)

Build this first (names are canonical; Vortex may alias):

| Target tool | Vortex today | Notes |
|-------------|--------------|-------|
| `glob_files` | `list_files` | via `project_index` |
| `grep_code` | `search_project` | context lines, caps |
| `repo_map` | — | **not built** |
| `read_slice` | `read_file` + `start_line`/`end_line` | rename/split optional |
| `symbols` | — | tree-sitter or LSP |
| `definition` / `references` | — | LSP |
| `edit_file` / `apply_patch` | yes | preview + approval |
| `run_command` | `bash_virtual` / `run_real_command` | cap output |
| `todo_write` | yes | |

Do **not** start with embeddings/RAG. For dev agents, `rg` + repo map + LSP + line slices are more predictable and token-efficient.

---

## Rule for the system prompt

```txt
Context is expensive. Never read full files unless the user explicitly asks or the file is tiny.

For every coding task:
1. Use repo_map (when available), list_files, search_project, or LSP before read_file slices.
2. Read only the smallest line ranges needed (start_line/end_line).
3. Prefer search results and symbol summaries over raw source.
4. Keep raw command output out of context; summarize failures.
5. For simple tasks, use small_task_mode: max 3 discovery calls and max 2 file slices before editing.
6. After editing, validate with the narrowest command first.
7. If output is large, store it externally and return only the relevant lines.
```

Implemented in `crates/agent_context/src/prompt.rs` (`SYSTEM_PROMPT`).

---

## Target architecture

```txt
┌──────────────────────────┐
│ GPUI Frontend             │  crates/app
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Agent Runtime             │  agent_core — loop, permissions, task state
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Context Manager           │  agent_context — budget, compaction, summaries
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Code Intelligence Layer   │  project_index (+ future repo map, LSP)
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Tool Executor             │  agent_tools
└────────────┬─────────────┘
             │
┌────────────▼─────────────┐
│ Session Store             │  agent_store — SQLite, raw outputs, history
└──────────────────────────┘
```

---

## Biggest fixes for token usage (priority)

1. Replace full `read` with slice-first policy (`read_file` + line range).
2. Add `repo_map` before any file read.
3. Enforce `search_project` with small `context_before`/`context_after` and `max_hits`.
4. Add `symbols(path)` (tree-sitter or LSP).
5. Store raw tool output outside the prompt (summaries in context only).
6. Add small task mode (runtime enforcement).
7. Hard per-turn token budgets (`ContextBudget` in `agent_protocol`).
8. Summarize build logs and command outputs.
9. Cache file summaries for repeated reads (`read_file` cache in `agent_core`).
10. UI: token cost per tool call (visibility for waste).

---

## Vortex implementation status

| Area | Status | Location |
|------|--------|----------|
| Slice reads | Partial | `read_file` — `start_line`/`end_line`, 1500 lines / 96KB cap |
| Discovery search | Yes | `search_project` — `project_index` / ripgrep |
| File listing | Yes | `list_files` |
| Context budget | Partial | `ContextBudget`, `fit_history`, `cap_message_content` in `agent_context` |
| Read dedupe cache | Yes | `agent_core` runtime per-run `read_file` cache |
| Skip UI stream for reads | Yes | `tool_exec.rs` — large outputs not streamed to thread UI |
| Repo map | Planned | — |
| LSP tools | Planned | — |
| External tool result store | Partial | SQLite events; not yet summary-only prompt injection |
| Small task mode | Prompt only | enforce in runtime later |
| Per-tool token cost in UI | Planned | — |

---

## Related docs

- [`docs/edit-transaction-contract.md`](./edit-transaction-contract.md) — batch edits, patch limits, atomic apply
- [`AGENTS.md`](../AGENTS.md) — workspace guide and runtime rules
- [`crates/agent_tools/AGENTS.md`](../crates/agent_tools/AGENTS.md) — adding tools
- [`docs/thread-performance.md`](./thread-performance.md) — UI streaming performance
