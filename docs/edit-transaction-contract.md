# Edit transaction contract

Poor editing usually comes from the agent acting too incrementally:

```txt
read -> patch -> patch failed -> read again -> patch nearby -> patch again -> formatter breaks -> patch again
```

The fix is an **edit transaction layer** — edits behave like a compiler transaction, not a stream of tiny patch calls.

Use this document as the product and architecture spec for Vortex editing. Live policy rules are in `crates/agent_context/src/prompt.rs`.

---

## What mature tools do

**Aider** supports multiple edit formats (diff-style and whole-file). Whole-file edits cost more latency than diffs. Its **Architect/Editor** mode separates reasoning from editing: one model plans changes, another narrowly applies file edits. ([Aider edit formats](https://aider.chat/docs/more/edit-formats.html))

**OpenCode** exposes separate `edit`, `write`, and `apply_patch` under one edit permission. `apply_patch` supports multi-file patch text (add, update, move, delete) — one atomic patch can express a whole change. ([OpenCode tools](https://opencode.ai/docs/tools/))

**Claude Code** emphasizes code intelligence, LSP navigation, subagents, and isolated context summaries so the main agent does not blindly read and modify files. ([Claude Code features](https://code.claude.com/docs/en/features-overview))

---

## Best fix: edit transaction layer

Do not let the model spam `apply_patch` or chain many `edit_file` calls on the same file without validation.

The agent should produce:

```ts
type EditTransaction = {
  goal: string;
  files: FileEdit[];
  validation: ValidationPlan;
};

type FileEdit = {
  path: string;
  baseHash: string;
  edits: TextEdit[];
};

type TextEdit = {
  startLine: number;
  endLine: number;
  oldText?: string;
  newText: string;
  reason: string;
};
```

Runtime pipeline:

```txt
1. Validate baseHash.
2. Sort edits by file and line range.
3. Detect overlaps.
4. Merge adjacent edits.
5. Dry-run patch.
6. Show/record unified diff.
7. Apply once.
8. Format affected files.
9. Run narrow validation.
```

The model submits **one edit transaction**, not eight patch calls.

---

## Target tools

### 1. `propose_edit_plan`

No file modification. Returns target files and intended changes only.

```ts
propose_edit_plan({
  task: string,
  knownFiles: string[],
  constraints: string[]
})
```

Example output:

```txt
Need to edit:
- src/App.tsx: wire todo state
- src/components/TodoInput.tsx: add controlled input
- src/components/TodoList.tsx: render items

No need to edit:
- package.json
- routing
- auth
```

### 2. `apply_batch_edit` / `apply_edit_transaction`

All edits at once:

```ts
apply_edit_transaction({
  goal: string,
  files: [
    {
      path: "src/App.tsx",
      baseHash: "abc123",
      mode: "exact",
      replacements: [
        {
          oldText: "const [todos, setTodos] = useState([]);",
          newText: "const [todos, setTodos] = useState<Todo[]>([]);"
        }
      ]
    }
  ],
  options: {
    dryRunFirst: true,
    formatChangedFiles: true,
    maxChangedLines: 300,
    rejectUnrelatedChanges: true
  }
})
```

Compact result (not full logs):

```txt
Applied:
- src/App.tsx: 1 replacement
- src/components/TodoInput.tsx: 2 replacements

Formatted:
- src/App.tsx

Validation:
- typecheck failed at src/App.tsx:42
```

### 3. `dry_run_edit`

Before apply, return unified diff only for self-check.

### 4. `semantic_edit`

AST/LSP-backed operations for fragile refactors:

```txt
rename_symbol
move_file
add_import
remove_import
replace_function_body
insert_component_prop
add_route
add_test_case
```

Text patches are fragile on large files; semantic edits are more stable.

---

## Editing strategy by file size

```txt
File < 150 lines:
  whole_file_replace is acceptable.

File 150–800 lines:
  line-range batch edit (exact replacements).

File > 800 lines:
  symbol-level edit only; no whole-file rewrite.
```

Hybrid policy:

```txt
small file  -> write_file / whole replace
medium file -> batch exact replacements (edit_file coalesced)
large file  -> LSP/AST semantic_edit only
```

---

## Runtime hard limits

```txt
Max patch attempts per file: 2
Max patch tool calls per task before validation: 1
Max failed patch retries before fallback: 1
No second patch without reading the exact affected slice
No patch after formatter/typecheck failure until error is summarized
```

**Fallback:**

```txt
Patch failed once:
  read ~20 lines around failed range; retry exact replacement.

Patch failed twice (small file):
  switch to write_file / whole-file replace.

Patch failed twice (large file):
  semantic_edit or locate symbol via search/LSP.
```

---

## Patch quality scoring

Before apply, score the edit. Reject or warn when:

```txt
- patch touches unrelated files
- huge unrelated formatting churn
- unexpected large deletes
- edits generated files (dist/, *.g.dart, lockfiles)
- TODO placeholders in production code
- tests changed without implementation (or reverse) when both expected
- public API changed without call-site updates
```

```ts
type PatchQuality = {
  risk: "low" | "medium" | "high";
  changedFiles: number;
  changedLines: number;
  unrelatedChanges: string[];
  missingLikelyFiles: string[];
  recommendation: "apply" | "review" | "reject";
};
```

---

## Two-model edit pipeline (optional)

```txt
Planner model:
  understands task, chooses files, writes compact edit instructions

Editor model:
  receives only target file slices
  produces exact batch edit
```

Mirrors Aider Architect/Editor: split reasoning from precise editing. Editor can be a cheaper model tuned for structured diffs.

---

## Versioned files (baseHash)

Every `read_file` should eventually return:

```txt
src/App.tsx
hash: sha256:abc123
lines: 1-120
```

Every edit must include `baseHash`. On mismatch:

```txt
Edit rejected: baseHash mismatch.
Current hash: def456.
Required action: refresh slice before editing.
```

Prevents stale patches after format or prior edits.

---

## Import handling

Dedicated tools (not manual import shuffling in patches):

```ts
add_import(path, importSpec)
remove_unused_imports(path)
organize_imports(path)
```

Post-edit pipeline:

```txt
1. apply edit transaction
2. organize imports (changed files only)
3. format changed files
4. narrow typecheck/test
```

---

## Validation output

Bad: `run npm test` → 8,000 lines in context.

Good:

```ts
run_validation({
  command: "cargo test -p app",
  maxErrors: 5,
  relevantFiles: ["src/App.tsx", "src/store/todos.ts"]
})
```

Return top errors + likely cause + suggested slice to read next.

---

## Edit memory (per file)

```txt
src/store/todos.ts
- owns Todo type
- exports createTodo, toggleTodo, deleteTodo
- tests in src/store/todos.test.ts
- last edited for priority support
```

Avoid rediscovering the same facts via repeated full reads.

---

## Special optimizations

| Technique | Purpose |
|-----------|---------|
| Patch coalescing | Merge multiple `edit_file` intents on same file into one transaction |
| Range locking | Edits only in lines last read unless wider range requested |
| Changed-file budget | Small task: ≤3 files, ≤120 lines, 1 transaction |
| Intent-specific modes | typo → exact replace; feature → batch; refactor → semantic; new app → `write_file` |
| Generated file detection | Block `dist/`, `build/`, `*.freezed.dart`, lockfiles (use package manager) |
| One-shot creation | New files via `write_file`, not empty-file line patches |

---

## System prompt rule (editing policy)

```txt
Editing policy:

Prefer one coherent edit transaction over many small patches.

Before editing:
- identify the smallest set of files
- read only the relevant slices
- create a short edit plan (mentally or via todo_write)
- choose mode: write_file (new/small), edit_file batch (medium), propose_patch multi-file (when diff is clearest)

During editing:
- batch all changes per file; coalesce multiple edits into one proposal before apply_patch
- avoid touching unrelated code
- never patch the same file repeatedly without validation feedback
- do not chain edit_file → edit_file → edit_file on one file without reading the failed slice

After editing:
- apply_patch once per approved batch when possible
- run the narrowest validation command
- summarize only relevant errors
- do not continue patching blindly after failed validation
```

---

## Ideal edit flow

```txt
User task
  ↓
Classify task size (small / medium / large)
  ↓
Repo map / list_files / search_project
  ↓
Read minimal slices
  ↓
Edit plan (files + intent)
  ↓
Dry-run batch (propose_patch / edit_file previews)
  ↓
Apply once (apply_patch + checkpoint)
  ↓
Format changed files (future: formatter tool)
  ↓
Validate narrowly
  ↓
Fix only reported validation errors
```

---

## Main primitive (target)

```ts
apply_edit_transaction({
  goal: string,
  files: FileEdit[],
  options: {
    dryRunFirst: true,
    formatChangedFiles: true,
    maxChangedLines: 300,
    rejectUnrelatedChanges: true
  }
})
```

Eventually **restrict** raw `apply_patch` to human-approved multi-file diffs; day-to-day edits go through the transaction API.

The agent should think in **atomic code changes**, not “patch calls”.

---

## Vortex today vs target

| Capability | Today | Target |
|------------|-------|--------|
| Structured single-file edit | `edit_file` (old/new → unified diff proposal) | Coalesce into batch |
| Whole-file write | `write_file` | Same; prefer for new files |
| Multi-file unified diff | `propose_patch` → preview → `apply_patch` | `apply_edit_transaction` |
| Checkpoints before apply | Yes (`ApplyPatchTool` + `checkpoint_dir`) | Keep |
| baseHash on read | No | Add to `read_file` output |
| `propose_edit_plan` tool | No | Add (read-only) |
| `dry_run_edit` | Partial (`propose_patch` preview only) | Explicit dry-run on transaction |
| `semantic_edit` / LSP | No | Add |
| Patch attempt limits | No (only `max_patch_bytes` in `AgentRunLimits`) | Runtime counters per file/task |
| Patch quality scoring | No | Pre-apply gate |
| Import tools | No | `add_import`, `organize_imports` |
| Narrow validation summarizer | No | Cap command output in context |
| Architect/Editor split | `delegate` subagent (partial) | Dedicated planner/editor roles |
| Edit memory | No | Per-file facts in session store |

**Existing pipeline:** `edit_file` / `write_file` / `delete_file` → `make_patch_proposal` in `patch_engine.rs` → user preview → `apply_patch` with checkpoint. Multiple `edit_file` calls per turn are possible today — the transaction layer should merge and enforce limits.

**Code locations:**

- Patch engine: `crates/agent_tools/src/shared/patch_engine.rs`
- Tools: `crates/agent_tools/src/tools/{edit_file,write_file,propose_patch,apply_patch}/`
- Runtime apply: `crates/agent_core/src/runtime/tool_exec.rs`
- System prompt: `crates/agent_context/src/prompt.rs`

---

## Related docs

- [`docs/context-and-tool-contract.md`](./context-and-tool-contract.md) — search-first, slice reads, context budget
- [`AGENTS.md`](../AGENTS.md) — runtime rules (patches previewable, checkpoints)
- [`crates/agent_tools/AGENTS.md`](../crates/agent_tools/AGENTS.md) — adding tools
