# C.I.S. Migration Documentation

## Overview

C.I.S. (Context & Intelligence System) is being migrated from a legacy Python implementation to a
Rust-first architecture. The Rust codebase is now the **primary and sole implementation**. The old
Python analysis engine and React web UI are preserved as **legacy reference only** and must not be
maintained as parallel implementations.

## Phase 0 Summary

This document records what was done during Phase 0 — Repository Audit and Structure Correction.

### What Was Moved

The Rust project was relocated from a deeply nested path to the repository root:

| From (old, nested)                                  | To (new, repo root)                     |
|-----------------------------------------------------|-----------------------------------------|
| `C.I.S\C.I.S\cis\Cargo.toml`                        | `Cargo.toml`                            |
| `C.I.S\C.I.S\cis\Cargo.lock`                        | `Cargo.lock`                            |
| `C.I.S\C.I.S\cis\src\` (all source files)           | `src\`                                  |
| `C.I.S\C.I.S\cis\tests\` (integration tests)        | `tests\`                                |

After moving these files, the `C.I.S\C.I.S\cis\` directory contained only the `target/` build
artifact directory and was deleted.

**Repository root (where Phase 0 was executed):**

```
C:\Users\Admin\Desktop\C.I.S\
├── Cargo.toml           ← Rust project root (moved)
├── Cargo.lock           ← Rust lock file (moved)
├── src/                 ← Rust source (moved)
├── tests/               ← Rust integration tests (moved)
├── .cisignore           ← C.I.S. scanner ignore patterns (created)
├── CIS_UPDATED_README.md ← Original specification (untouched)
├── REVISED_IMPLEMENTATION_PLAN.md ← Revised plan (untouched)
├── docs/                ← Documentation (created)
├── C.I.S/               ← Legacy project folder (preserved, see below)
```

### What Was Intentionally NOT Moved (Phase 0)

Per the implementation plan, legacy code must not be moved or deleted in Phase 0. The following
directories remain at their original nested location inside `C.I.S\C.I.S\`:

| Path (relative to repo root)            | Description                                   | Status        |
|-----------------------------------------|-----------------------------------------------|---------------|
| `C.I.S\C.I.S\codebase_analyzer\`         | Legacy Python analysis engine + FastAPI server | Preserved     |
| `C.I.S\C.I.S\codeintel_ui\`             | Legacy React/Vite web UI + FastAPI bridge     | Preserved     |
| `C.I.S\C.I.S\.gitignore`                | Legacy gitignore (only `.env` patterns)       | Preserved     |
| `C.I.S\C.I.S\README.md`                 | Legacy README referencing old `cis/` path     | Preserved     |
| `C.I.S\C.I.S\.git\`                     | Git repository for the legacy project         | Preserved     |
| `C.I.S\C.I.S\.cis\`                     | Runtime `.cis/` data from a prior scan        | Preserved     |

These directories are **reference-only**. They must not be developed, extended, or maintained as a
parallel implementation. The Rust codebase is the single source of truth going forward.

### Component Classification

| Component                          | Classification | Notes                                                        |
|------------------------------------|----------------|--------------------------------------------------------------|
| `src/scanner/mod.rs`               | Rust (primary) | Replaces `backend/scanner.py` — Rust-native directory walker   |
| `src/parser/`                      | Rust (primary) | Replaces `backend/dependency_analyzer.py` — regex-based      |
| `src/index/mod.rs`                 | Rust (primary) | SQLite+FTS5 persistence (new, no Python equivalent)          |
| `src/config/mod.rs`                | Rust (primary) | TOML config management (new, no Python equivalent)            |
| `src/cli/mod.rs`                   | Rust (primary) | CLI entry point (new, no Python equivalent)                   |
| `src/project/mod.rs`               | Rust (primary) | Project root detection + `.cis/` management (new)             |
| `tests/integration_tests.rs`       | Rust (primary) | Integration tests (new)                                        |
| `codebase_analyzer/backend/scanner.py`   | Reference    | Port ignore rules, language map, categorization to Rust      |
| `codebase_analyzer/utils/file_handler.py` | Reference  | Port zip-slip protection pattern to Rust if needed            |
| `codebase_analyzer/backend/dependency_analyzer.py` | Replaceable | Import extraction rewritten in Rust (`src/parser/`) |
| `codebase_analyzer/backend/risk_engine.py` | Replaceable  | Weighted scoring logic to port if needed                      |
| `codebase_analyzer/backend/graph_renderer.py` | Replaceable | Replace with Rust-native approach (Phase 5+)                 |
| `codebase_analyzer/backend/execution_tracer.py` | Obsolete | Python-only; replaced by Rust terminal module (Phase 1)       |
| `codebase_analyzer/ui/main_window.py` | Obsolete        | 838-line Textual monolith; replaced by CLI + future TUI      |
| `codebase_analyzer/server.py`      | Broken          | Missing FastAPI/pydantic/py-dotenv deps; replaced by Rust CLI + MCP |
| `codeintel_ui/src/App.jsx`         | Obsolete        | 40 KB+ monolithic React component; rewrite later               |
| `codeintel_ui/server.py`           | Broken          | Broken `analyzer` import; no package.json deps for Python     |
| `codebase_analyzer/ai/`            | Obsolete/Remove | NIM AI client; violates zero-AI core policy                    |

### What Was Intentionally NOT Migrated

Per the zero-AI core principle, the following were deliberately excluded:

- **AI client modules** (`codebase_analyzer/ai/nim_client.py`, `prompt_engine.py`, `context_builder.py`):
  These embed an NVIDIA NIM AI client. The Rust core does not include any AI model or API client.
  External AI integration happens only via MCP (Phase 9).
- **React web UI** (`codeintel_ui/src/`): The UI is deferred to Phase 12. Future UI options include
  a terminal TUI (ratatui), a new web app, or a Tauri desktop app — all of which consume deterministic
  core data rather than replacing it.
- **FastAPI server** (`codebase_analyzer/server.py`, `codeintel_ui/server.py`): Replaced by the Rust
  CLI binary and MCP server (Phase 9).

## Git State

The implementation plan states that the repository is "not a Git working tree." The audit confirms a
**nuance**:

- `C:\Users\Admin\Desktop\C.I.S\.git` — **does not exist**. The repository root is not a Git working
  tree.
- `C:\Users\Admin\Desktop\C.I.S\C.I.S\.git` — **exists**. The inner `C.I.S\C.I.S\` folder is a Git
  working tree with:
  - Branch: `main` (up to date with `origin/main`)
  - Remote: `https://github.com/Rainox-cmd/C.I.S.git`
  - The `cis/` directory was **untracked** in this Git repository (confirmed at audit time).
  - Moving/deleting `cis/` did not create any Git state changes.

Git was **not** initialized at the repository root during Phase 0. The existing inner Git repository
is preserved for legacy history. A decision on whether to initialize Git at the new root or relocate
the existing `.git` directory is deferred to the user.

## Validation

- `cargo build` — succeeds from the repository root.
- `cargo test` — 26 unit tests + 5 integration tests = **31 tests pass** from the repository root.
- `.cisignore` — created at the repository root with sensible default patterns.
- `.cisignore` patterns verified: scanner skips `.git/`, `.cis/`, `target/`, `node_modules/`,
  `__pycache__/`, `venv/`, `.venv/`, `dist/`, `build/`, `out/`, and other common directories.
- Legacy Python/UI directories — confirmed untouched and intact.
