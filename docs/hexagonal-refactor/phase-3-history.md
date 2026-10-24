# Phase 3 (history): preparation

Status: **not started.** One run was launched at `c7b796e` and stopped before it edited or committed anything, so
Phase 3 starts from the end of Phase 2 (`c7b796e`, gate green).

Plan reference: [the plan](../hexagonal-refactor-plan.md), section 5 "Phase 3: history" and the `history` layout
in section 4. Detailed traps per legacy file: [`source-map/inline.md`](source-map/inline.md) (owner of `History`
today) and [`source-map/types.md`](source-map/types.md) (owner of `HistoryEntry`). Their line numbers are at
`af1b304`; the code below is checked against `c7b796e`.

## Where the code is today (`c7b796e`)

| What | Where | Used by |
|---|---|---|
| `HistoryEntry` (serde derives, all fields `pub`) | `src/types.rs:177-188` | `tracker.rs` (builds it with `..Default::default()` at 738, mutates `exit_code`/`end_time`), `engine.rs` (`llm_history` 158, tests 1259), `suggest.rs` (`history_rows` 349, tests 1890, 2358), `inline.rs` |
| `History` + `HISTORY_IN_MEMORY` (5000) + `HISTORY_COMPACT_AT` (10 000) + `open_private` | `src/inline.rs:17-135` | `engine.rs` (load in `Engine::new`, append on `CommandFinished`, `entries()` per keystroke and per LLM request) |
| `history_file_round_trip_and_compaction` | `src/inline.rs:1057` | the adapter-level test that moves with `History` |

`History::load` already resolves its file through `data_directory_from_env().history_file()` (Phase 2).

## Behaviour that must not change

Wire format of `~/.tabsmith/history.jsonl`:
- One `serde_json::to_string(&entry)` line per entry, field order `command, shell, pid, session_id, cwd, start_time,
  end_time, exit_code, hostname`; `None` is written as `null` (no `skip_serializing_if`).
- No `#[serde(default)]`: a line missing `command`, `shell`, `session_id` or `start_time` is dropped by
  `filter_map(|l| serde_json::from_str(l).ok())`; unknown fields are ignored. Adding defaults would start loading
  lines that are skipped today.
- `shell` is the raw OSC marker string from the tracker, not the `Shell` VO (typing it would normalise or drop
  values). `hostname` is never set. `start_time`/`end_time` are seconds.

Loading and appending:
- `read_to_string(..).unwrap_or_default()`: any read error, including one invalid UTF-8 byte anywhere, gives an empty
  history. Keep it; per-line lossy decoding would change it.
- Memory keeps the last 5000 entries (`HISTORY_IN_MEMORY`); the file is compacted when the line count is
  `> 10_000` (strict), down to its last 5000 lines.
- Compaction writes `history.tmp-<pid>` (via `with_extension`) and renames it over the file. That **replaces** a
  symlinked `history.jsonl` with a regular file and turns CRLF into LF (`lines()` + `join("\n")`). Do not route it
  through `shared::AtomicFile`, which keeps symlinks. On error: warn `history: compacting {path}: {e}`.
- `open_private` sets mode `0600` only when it creates the file; an existing `0644` file stays `0644` across appends.
- Purge guard: when the parent dir is gone (`uninstall --purge` while a session runs), the write is skipped, the dir
  is not recreated and the line is not counted, but the entry is still pushed to memory.
- Write error: warn `history: appending to {path}: {e}`, still push to memory.
- No data directory: warn `history: {e:#}` and run memory-only. This memory-only fallback is a **production** path,
  so `InMemoryHistoryRepository` cannot be `#[cfg(test)]`-only, and its append keeps the 5000 cap.
- `entries()` is the load-time snapshot plus this process's own appends, oldest first. Other sessions' appends are
  invisible until restart. `recent` must never re-read the file (ghost text, history rows and LLM context depend on
  it, and it is read on every keystroke).
- The blank-command skip (`command.trim().is_empty()`) is asserted at the adapter by the moved test. Keep it there
  even though `RecordCommandUseCase` also skips blanks (D1).

Entity shape:
- `HistoryEntry` derives structural `PartialEq`, `Clone`, `Debug`, `Default`; tests depend on them
  (`assert_eq!(again.entries(), h.entries())` with fixtures whose `session_id` is `""` and `start_time` is 0).
  Identity-based equality would make that assertion vacuous. Expose the glossary identity (session id, start
  time) as an explicit method instead.
- Plan section 1 makes domain fields private, so the tracker's struct literal and field mutation need a constructor
  and intention-revealing transitions; the tests' struct literals switch to the constructor (allowed restructuring,
  same values).

## Step briefs

These are the prompts prepared for the three implementer agents. They read
[`agent-brief.md`](agent-brief.md) first.

1. **HistoryEntry entity without serde + HistoryRecord in the adapter.** Move `types::HistoryEntry` to
   `src/history/domain/entities/history_entry.rs` with no serde. The JSONL contract moves to a serde
   `HistoryRecord` in `src/history/infrastructure/adapters/jsonl_history_repository.rs` with exactly the wire
   format above. Private fields plus the constructor and transitions its users need; structural derives kept;
   `to_primitives()` only if the adapter mapping needs it. Switch every user (a `types.rs` re-export may stay as a
   shim only within this phase).
2. **HistoryRepository + InMemory + JsonlHistoryRepository.** Test-first:
   `src/history/domain/repositories/history_repository.rs` with `trait HistoryRepository { append, recent }` and a
   production `InMemoryHistoryRepository` (5000 cap). The engine thread owns the history today (`&mut`), so a
   `&self` trait needs interior mutability (`RefCell` is enough on one thread). Then move `inline::History`
   (load, load_from, append, compact, open_private, the constants) into `JsonlHistoryRepository` verbatim, with
   `history_file_round_trip_and_compaction`. Path from `DataDirectory::history_file` (lazy, D3). Switch
   `engine.rs` and `inline.rs` to the trait or the adapter.
3. **RecordCommandUseCase + history_module + register history.** Test-first `RecordCommandUseCase` (skip blanks,
   else append). `history_module.rs` with `build_history_module(&SharedDependencies)` keeping the memory-only
   fallback. `history_error.rs` only if something is fallible. Switch the engine's `CommandFinished` path (built on
   the engine thread: QuickJS is `!Send`, keep the threads as they are). Delete `inline::History` and the
   `types::HistoryEntry` shim. Register `history` in `REGISTERED_MODULES`.

## Launch

From the repo root, with the gate green:

```js
Workflow({
  scriptPath: "docs/hexagonal-refactor/phase-workflow.js",
  args: {
    phase: "Phase 3 (history)",
    startRev: "<HEAD when you launch>",
    reviewNotes: "Phase 3 builds the history module (HistoryEntry entity, HistoryRepository + InMemory, JsonlHistoryRepository, RecordCommandUseCase, history_module) out of types::HistoryEntry and inline::History, and registers `history` in the architecture test. inline.rs (ghost text, LLM) stays until Phase 7 and engine.rs until Phase 6; they switch to the new history types/ports but keep their own logic. Read docs/hexagonal-refactor/phase-3-history.md and source-map/inline.md + types.md. Do not demand later-phase work.",
    steps: [ /* the three briefs above, each as {id, title, prompt}; tell each agent to read this file first */ ]
  }
})
```
