# C.I.S. Migration Documentation

## Overview

C.I.S. (Context & Intelligence System) was migrated from a legacy Python implementation to a
Rust-first architecture. The Rust codebase is now the **primary and sole implementation**. The old
Python analysis engine and React web UI have been **removed** from the working tree.

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

### Legacy Code Removal

The legacy Python codebase (`C.I.S/C.I.S/codebase_analyzer/`) and React web UI
(`C.I.S/C.I.S/codeintel_ui/`) have been removed from the working tree. All Rust functionality is
self-contained and has no dependency on the legacy code. Git history for the legacy code is
preserved in the `C.I.S/C.I.S` submodule repository.

### Component Classification

| Component                          | Classification | Notes                                                        |
|------------------------------------|----------------|--------------------------------------------------------------|
| `src/scanner/mod.rs`               | Rust (primary) | Directory walker                              |
| `src/parser/`                      | Rust (primary) | Regex-based import extraction                 |
| `src/index/mod.rs`                 | Rust (primary) | SQLite+FTS5 persistence                       |
| `src/config/mod.rs`                | Rust (primary) | TOML config management                        |
| `src/cli/mod.rs`                   | Rust (primary) | CLI entry point                               |
| `src/project/mod.rs`               | Rust (primary) | Project root detection + `.cis/` management   |
| `tests/integration_tests.rs`       | Rust (primary) | Integration tests                             |
| ~~`codebase_analyzer/backend/scanner.py`~~   | Removed (legacy) | Replaced by Rust `src/scanner/` (see below)    |
| ~~`codebase_analyzer/utils/file_handler.py`~~ | Removed (legacy) | Port zip-slip protection (not needed in Rust) |
| ~~`codebase_analyzer/backend/dependency_analyzer.py`~~ | Removed (legacy) | Replaced by `src/parser/` |
| ~~`codebase_analyzer/backend/risk_engine.py`~~ | Removed (legacy) | Weighted scoring (future Rust port)         |
| ~~`codebase_analyzer/backend/graph_renderer.py`~~ | Removed (legacy) | Replace with Rust-native approach      |
| ~~`codebase_analyzer/backend/execution_tracer.py`~~ | Removed (legacy) | Replaced by Rust terminal module  |
| ~~`codebase_analyzer/ui/main_window.py`~~ | Removed (legacy) | Replaced by CLI + future TUI          |
| ~~`codebase_analyzer/server.py`~~ | Removed (legacy) | Replaced by Rust CLI + MCP server         |
| ~~`codeintel_ui/src/App.jsx`~~     | Removed (legacy) | Rewrite deferred to Phase 12                |
| ~~`codeintel_ui/server.py`~~       | Removed (legacy) | Broken; replaced by Rust CLI + MCP          |
| ~~`codebase_analyzer/ai/`~~        | Removed (legacy) | NIM AI client; replaced by MCP integration |

### Legacy Code Replaced

Per the zero-AI core principle, the following legacy components were **replaced** by Rust:

- **AI client modules** (`codebase_analyzer/ai/nim_client.py`, `prompt_engine.py`, `context_builder.py`):
  These embedded an NVIDIA NIM AI client. The Rust core has zero AI dependencies. External AI
  integration is handled via MCP (Phase 9).
- **React web UI** (`codeintel_ui/src/`): Deferred to Phase 12. Future UI options include
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
- Legacy Python/UI directories — **removed** from the working tree. All functionality replaced by Rust C.I.S.
