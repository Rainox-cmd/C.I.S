# Known Issues — Legacy Python/UI Implementation

This document records issues discovered during the Phase 0 audit of the legacy Python and React
implementations. These issues are documented for historical reference only. Per the implementation
plan, legacy bugs are **not fixed** in Phase 0. The Rust codebase is the primary implementation
going forward.

---

## 1. codebase_analyzer/server.py — Missing Dependencies

**Severity:** High (server cannot start)

The file `codebase_analyzer/server.py` imports modules that are **not listed** in
`codebase_analyzer/requirements.txt`:

| Import                                   | Source package (missing from requirements.txt) |
|------------------------------------------|-----------------------------------------------|
| `from dotenv import load_dotenv`         | `python-dotenv` (not listed)                  |
| `from fastapi import ...`                | `fastapi` (not listed)                        |
| `from pydantic import BaseModel`         | `pydantic` (not listed)                       |
| `from fastapi.responses import ...`      | `fastapi` (not listed)                        |
| `from fastapi.middleware.cors import ...` | `fastapi` (not listed)                       |

The `requirements.txt` only lists: `textual`, `rich`, `networkx`, `matplotlib`, `openai`.

The README instructs users to manually install additional packages (`fastapi uvicorn networkx
matplotlib openai`), but `python-dotenv` is never mentioned. This means the `/analyze` and `/chat`
endpoints cannot function out of the box.

## 2. codeintel_ui/server.py — Broken `analyzer` Import

**Severity:** High (server cannot start)

The file `codeintel_ui/server.py` (line 13) contains:

```python
from analyzer import analyze_project
```

No `analyzer` module, package, or `analyzer.py` file exists anywhere in `codeintel_ui/` or in the
Python path that `server.py` sets up (`sys.path.insert(0, str(Path(__file__).parent))`). This import
will fail with `ModuleNotFoundError: No analyzer module named 'analyzer'`.

The entire `/analyze` endpoint (which calls `analyze_project`) is therefore non-functional.

## 3. codeintel_ui/server.py — Missing Python Dependencies

**Severity:** High (server cannot start)

`codeintel_ui/server.py` imports `fastapi`, `pydantic`, `uvicorn`, and references `ai.nim_client`
and `ai.prompt_engine` (lines 80–81). None of these are available:

- `codeintel_ui/package.json` only declares Node.js dependencies (React, Vite, D3, Three.js, etc.).
  There is no `requirements.txt` or equivalent Python dependency manifest for `codeintel_ui/`.
- The `ai/` directory exists only under `codebase_analyzer/`, not under `codeintel_ui/`. The
  `sys.path` insertion at line 12 only adds `codeintel_ui/` itself, so `from ai.nim_client import
  NIMClient` will fail with `ModuleNotFoundError`.

## 4. .gitignore — Incomplete Coverage

**Severity:** Medium (affects build hygiene, not runtime)

The `.gitignore` at `C.I.S\C.I.S\.gitignore` contains only `.env` patterns:

```
.env
.env
codebase_analyzer/.env
.env
```

It does **not** ignore:
- `target/` (Rust build artifacts)
- `node_modules/` (npm/yarn dependencies)
- `__pycache__/` (Python bytecode caches)
- `*.pyc`
- `.mypy_cache/`, `.pytest_cache/`, `.tox/`
- Coverage output (`htmlcov/`, `.coverage`)
- IDE directories (`.vscode/`, `.idea/`)
- OS files (`.DS_Store`)

This means these directories may be tracked in Git or clutter the working tree.

## 5. README.md — Stale Path References

**Severity:** Low (documentation only)

`C.I.S\C.I.S\README.md` references the old nested Rust path in several places (Section 6, "Rust CLI
(Phase 1 - Foundation)"):

```bash
cd cis
cargo build --release
...
.\cis\target\release\cis.exe init
```

These paths reflect the old `C.I.S\C.I.S\cis\` layout and are now outdated after Phase 0 moves the
Rust project to the repository root.

## 6. codeintel_ui/src/App.jsx — Monolithic Component

**Severity:** Low (maintainability)

`codeintel_ui/src/App.jsx` is a single 40,661-byte (≈40 KB) React component containing routing,
state management, API calls, and UI for multiple views (dashboard, file explorer, dependency graph,
chat, error console). It is flagged as "Obsolete" in the component classification table; a future
rewrite (Phase 12) should decompose it into smaller components.

## 7. codebase_analyzer/ai/ — Embedded AI Client

**Severity:** N/A (by design — obsolete)

The `codebase_analyzer/ai/` directory contains:
- `nim_client.py` — NVIDIA NIM API client
- `prompt_engine.py` — AI prompt construction
- `context_builder.py` — AI context assembly

These modules are classified as **Obsolete/Remove** because the zero-AI core policy mandates no
embedded AI models or API clients in the Rust implementation. External AI integration is handled
exclusively via MCP (Phase 9).

## 8. codebase_analyzer/ui/panels/chat_panel.py — AI Chat Coupling

**Severity:** Low (by design — obsolete)

The chat panel directly instantiates and calls `NIMClient` and `PromptEngine` from the `ai/`
subpackage. If the AI modules are removed or deprecated, this panel will fail at import time. This
is expected since the entire UI is being replaced by a CLI + MCP architecture.

---

## Summary Table

| # | File / Module                                  | Issue                                      | Severity |
|---|------------------------------------------------|--------------------------------------------|----------|
| 1 | `codebase_analyzer/server.py`                  | Missing fastapi, pydantic, python-dotenv   | High     |
| 2 | `codeintel_ui/server.py`                       | Broken `from analyzer import`            | High     |
| 3 | `codeintel_ui/server.py`                       | Missing Python deps (fastapi, ai/)         | High     |
| 4 | `C.I.S/.gitignore`                             | Incomplete (no target/, node_modules/, …)  | Medium   |
| 5 | `C.I.S/README.md`                              | Stale `cd cis` path references           | Low      |
| 6 | `codeintel_ui/src/App.jsx`                     | 40 KB monolithic component              | Low      |
| 7 | `codebase_analyzer/ai/`                        | Embedded NIM AI client (violates core)    | N/A      |
| 8 | `codebase_analyzer/ui/panels/chat_panel.py`    | Hard dependency on ai/ modules           | Low      |
