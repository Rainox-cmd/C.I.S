# C.I.S. — Revised Implementation Plan: In-Place Migration

## 1. Current State Assessment

### 1.1 Repository Root
```
C:\Users\Admin\Desktop\C.I.S\
```

### 1.2 Existing Components Classification

| Component | Classification | Rationale |
|-----------|---------------|-----------|
| `codebase_analyzer/backend/scanner.py` | **Reusable** | Cleanest module; good reference for ignore rules, language map, file categorization, line-counting with encoding fallback |
| `codebase_analyzer/utils/file_handler.py` | **Reusable** | ZIP extraction with zip-slip protection (good security pattern) |
| `codebase_analyzer/utils/logger.py` | **Reusable** | Logging utility |
| `codeintel_ui/src/components/DependencyGraph.jsx` | **Legacy/Reference** | Graph visualization concept; will be rewritten for Rust API |
| `codeintel_ui/src/components/ThreeBackground.jsx` | **Legacy/Reference** | 3D background concept |
| `codebase_analyzer/backend/dependency_analyzer.py` | **Replaceable** | Core logic to port; has dead `_resolve_relative` code |
| `codebase_analyzer/backend/risk_engine.py` | **Replaceable** | Weighted scoring model to port |
| `codebase_analyzer/backend/graph_renderer.py` | **Replaceable** | Matplotlib rendering to replace |
| `codebase_analyzer/backend/execution_tracer.py` | **Obsolete/Buggy** | Python-only; misnamed for multi-language; replace with Rust |
| `codebase_analyzer/backend/error_detector.py` | **Replaceable** | Error detection logic to port |
| `codebase_analyzer/main.py` | **Replaceable** | Textual UI entry; replace with Rust CLI + future TUI |
| `codebase_analyzer/ui/main_window.py` | **Obsolete/Buggy** | 725-line monolithic Textual panel; replace |
| `codebase_analyzer/server.py` | **Broken** | FastAPI backend with broken import (`from analyzer import analyze_project` doesn't exist); non-functional |
| `codeintel_ui/server.py` | **Broken** | Imports missing `analyzer` module; non-functional |
| `codeintel_ui/src/App.jsx` | **Obsolete/Buggy** | 838-line React component; dense inline styles; will be rewritten |
| `codeintel_ui/src/hooks/useAnalyzer.js` | **Replaceable** | Will be replaced with Rust API client |
| `codebase_analyzer/ai/` | **Obsolete/Remove** | NIM AI client, prompt engine, context builder — violates zero-AI core |
| `codebase_analyzer/requirements.txt` | **Obsolete/Buggy** | Missing `fastapi`, `uvicorn`, `pydantic`, `python-dotenv` |

### 1.3 Existing Bugs to Fix/Isolate
- `codebase_analyzer/server.py:13` — broken import (`from analyzer import analyze_project`)
- `codeintel_ui/server.py:13` — broken import
- `codebase_analyzer/backend/dependency_analyzer.py:_resolve_relative` — dead code
- `codebase_analyzer/backend/execution_tracer.py` — Python-only execution tracing
- `requirements.txt` — incomplete dependencies
- No tests exist anywhere in the repository
- Global state in `server.py:66` (`_last_results` dict)

## 2. Corrected Repository Structure

### 2.1 Target Structure (In-Place Migration)

```
C:\Users\Admin\Desktop\C.I.S\
├── Cargo.toml                 ← RUST PROJECT ROOT (moved from cis/)
├── Cargo.lock
├── src\                       ← RUST SOURCE (moved from cis/src/)
│   ├── main.rs
│   ├── cli\
│   ├── config\
│   ├── scanner\
│   ├── parser\
│   ├── index\
│   ├── git\
│   ├── memory\
│   ├── terminal\
│   ├── security\
│   ├── mcp\
│   └── diagnostics\
├── tests\                     ← RUST TESTS (moved from cis/tests/)
├── target\                    ← Build artifacts
├── .cisignore                 ← NEW: Scanner ignore file
├── CIS_UPDATED_README.md
├── README.md
├── docs\
├── scripts\
├── codebase_analyzer\         ← LEGACY PYTHON (preserved, deprecated)
│   ├── ai\                    ← ISOLATED AI (to be removed in Phase 0)
│   ├── backend\
│   ├── ui\
│   ├── utils\
│   ├── main.py
│   ├── server.py
│   ├── requirements.txt
│   └── __init__.py
└── codeintel_ui\              ← LEGACY WEB UI (preserved, will be updated later)
    ├── src\
    ├── server.py
    ├── package.json
    ├── vite.config.js
    ├── index.html
    └── node_modules\
```

### 2.2 What Changes
1. **Move `cis/Cargo.toml` → `Cargo.toml`** at repo root
2. **Move `cis/src/` → `src/`** at repo root
3. **Move `cis/tests/` → `tests/`** at repo root
4. **Delete empty `cis/` directory**
5. **Keep old Python/UI code in place** — preserves git history, no deletion
6. **Add `.cisignore`** at repo root for scanner ignore rules

### 2.3 What Does NOT Change
- Old Python code stays in `codebase_analyzer/`
- Old React UI stays in `codeintel_ui/`
- Git history preserved
- No code deletion without explicit approval

## 3. Migration Strategy

### Phase 0: Audit, Bug Fixing, and Structure Correction
**Goal:** Stabilize existing code and correct repository structure.

#### 0.1 Correct Repository Structure
- Move Rust code from `cis/` to repo root
- Verify `cargo build` works from root
- Update any internal paths

#### 0.2 Fix Critical Bugs
- Fix `codebase_analyzer/server.py` broken import
- Fix `codeintel_ui/server.py` broken import
- Document known issues in `docs/known_issues.md`

#### 0.3 Isolate AI Components
- Move `codebase_analyzer/ai/` → `codebase_analyzer/ai_legacy/` (or similar)
- Add deprecation notice to directory
- Ensure Rust core has zero AI dependencies

#### 0.4 Add Regression Tests
- Add tests for existing Python scanner logic
- Verify existing functionality before Rust replacement

### Phase 1: Rust Foundation (Root-Level)
**Goal:** Rust becomes the project's primary entry point.

- `Cargo.toml` at repo root
- `src/main.rs` as primary binary
- `cis init`, `cis scan`, `cis status`, `cis doctor` commands
- `.cis/` project-local storage
- SQLite + FTS5 schema
- All tests pass on Windows

### Phase 2-15: Continue as per CIS_UPDATED_README.md
- Scanner, Parser, Index, Git, Memory, Terminal, MCP, UI, etc.
- Each phase builds on the Rust core at repo root

## 4. Component Migration Map

| Existing Python Component | Rust Replacement | Reuse Strategy |
|--------------------------|-----------------|----------------|
| `scanner.py` | `src/scanner/` | Port ignore rules, language map, categorization |
| `dependency_analyzer.py` | `src/parser/` + `src/index/` | Port import extraction, graph logic |
| `risk_engine.py` | `src/diagnostics/` | Port weighted scoring |
| `execution_tracer.py` | `src/terminal/` | Replace with safe Rust execution |
| `graph_renderer.py` | `src/graph/` (future) | Replace with Rust graph library |
| `error_detector.py` | `src/diagnostics/` | Port error detection |
| `file_handler.py` | `src/utils/` | Reuse zip-slip protection pattern |
| `logger.py` | `src/utils/` | Reuse logging pattern |
| `main_window.py` | Future TUI (ratatui) | Rewrite |
| `server.py` | Eliminated | Rust CLI + MCP replace FastAPI |
| `ai/*` | Eliminated | Zero-AI core; external AI via MCP only |

## 5. UI Strategy

### 5.1 Legacy UI (`codeintel_ui/`)
- **Preserve** existing React UI as reference
- **Do not delete** — may contain useful component patterns
- **Do not actively develop** — will be rewritten for Rust API
- **Status:** Legacy/Reference

### 5.2 Future UI
- New UI (web or TUI) will consume Rust core via:
  - CLI output (JSON flags)
  - MCP tools
  - Future HTTP API if needed
- UI is Phase 12 per roadmap — not immediate priority

## 6. Zero-AI Core Enforcement

### 6.1 AI Components to Isolate
```
codebase_analyzer/ai/
├── context_builder.py    → Move to ai_legacy/
├── nim_client.py         → Move to ai_legacy/
├── prompt_engine.py      → Move to ai_legacy/
└── __init__.py
```

### 6.2 Rust Core AI Policy
- **No** embedded LLM
- **No** AI API keys in core
- **No** NIM/OpenAI/Anthropic clients
- **Yes** MCP server for external AI integration (Phase 10)
- **Yes** context budgeting and retrieval (Phase 8)

## 7. Next Steps

1. **Approve this revised plan**
2. **Execute Phase 0.1** — Move Rust code from `cis/` to repo root
3. **Execute Phase 0.2** — Fix critical Python bugs (optional, non-blocking)
4. **Execute Phase 0.3** — Isolate AI components
5. **Continue Phase 1** — Rust foundation at root level

## 8. Git History Preservation

- Old Python code: **never moved** — full history preserved
- Rust code: **new addition** — no history to preserve (created during migration)
- If Rust files must be moved later: use `git mv` to preserve history
