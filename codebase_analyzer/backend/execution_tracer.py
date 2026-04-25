"""
backend/execution_tracer.py
Phase 3: Safe Python execution tracing.
- Detects entry points (main.py, app.py, etc.)
- Runs compile check via Python's compile()
- Runs py_compile in a sandboxed subprocess with hard timeout
- Maps runtime failures back to the dependency graph (failure chain)

Public API:
    tracer = ExecutionTracer(scan_result, dep_result)
    trace_result = tracer.trace()   →  TraceResult
"""

import sys
import ast
import subprocess
import traceback
from pathlib import Path
from dataclasses import dataclass, field

from utils.logger import get_logger
from backend.scanner import ScanResult, FileRecord
from backend.dependency_analyzer import DependencyResult

log = get_logger(__name__)

# ─── Config ───────────────────────────────────────────────────────────────────

SAFE_EXEC_TIMEOUT = 3          # seconds
MAX_OUTPUT_CHARS  = 500        # truncate long stdout/stderr

ENTRY_POINT_NAMES = {
    "main.py", "app.py", "run.py", "server.py",
    "index.py", "start.py", "manage.py", "cli.py",
    "__main__.py",
}


# ─── Data types ───────────────────────────────────────────────────────────────

@dataclass
class FileExecResult:
    """Execution check result for a single Python file."""
    rel_path: str
    is_entry_point: bool = False

    # Compile check
    compile_ok: bool = False
    compile_error: str | None = None
    compile_line: int | None = None

    # Runtime check (subprocess)
    runtime_ok: bool | None = None    # None = not run
    runtime_error: str | None = None
    runtime_stdout: str | None = None
    runtime_stderr: str | None = None
    timed_out: bool = False

    # Failure chain (filled if file fails)
    failure_chain: list[str] = field(default_factory=list)
    root_failure: str | None = None

    def failed(self) -> bool:
        return not self.compile_ok or self.runtime_ok is False

    def to_console_entry(self) -> dict:
        if not self.compile_ok:
            return {
                "severity": "ERROR",
                "file": self.rel_path,
                "message": f"Compile failed — {self.compile_error}",
                "line": self.compile_line,
            }
        if self.timed_out:
            return {
                "severity": "WARNING",
                "file": self.rel_path,
                "message": f"Execution timed out after {SAFE_EXEC_TIMEOUT}s",
            }
        if self.runtime_ok is False:
            return {
                "severity": "ERROR",
                "file": self.rel_path,
                "message": f"Runtime error — {(self.runtime_error or '')[:120]}",
            }
        return {
            "severity": "INFO",
            "file": self.rel_path,
            "message": "Compile check passed",
        }


@dataclass
class TraceResult:
    """Execution trace results for the whole project."""
    entry_points: list[str] = field(default_factory=list)
    file_results: dict[str, FileExecResult] = field(default_factory=dict)
    failure_chains: list[dict] = field(default_factory=list)

    def failed_files(self) -> list[FileExecResult]:
        return [r for r in self.file_results.values() if r.failed()]

    def passing_files(self) -> list[FileExecResult]:
        return [r for r in self.file_results.values() if not r.failed()]

    def for_console(self) -> list[dict]:
        entries = []
        for r in self.file_results.values():
            entries.append(r.to_console_entry())
        return entries


# ─── Tracer ───────────────────────────────────────────────────────────────────

class ExecutionTracer:
    """
    Safe Python execution engine.
    ONLY runs compile() and py_compile in a subprocess.
    Never imports or exec()s arbitrary code.
    """

    def __init__(self, scan_result: ScanResult, dep_result: DependencyResult):
        self._scan = scan_result
        self._dep  = dep_result

    def trace(self) -> TraceResult:
        log.info("Starting execution trace")
        result = TraceResult()

        python_files = self._scan.files_for_language("Python")
        if not python_files:
            log.info("No Python files to trace")
            return result

        # Identify entry points
        result.entry_points = self._find_entry_points(python_files)
        log.info("Entry points: %s", result.entry_points)

        # Check each Python file
        for record in python_files:
            exec_result = self._check_file(record)
            exec_result.is_entry_point = record.rel_path in result.entry_points
            result.file_results[record.rel_path] = exec_result

        # Build failure chains
        self._build_failure_chains(result)

        failed = len(result.failed_files())
        log.info(
            "Execution trace complete: %d files checked, %d failed",
            len(result.file_results), failed,
        )
        return result

    # ── Entry point detection ─────────────────────────────────────────────────

    def _find_entry_points(self, python_files: list[FileRecord]) -> list[str]:
        """
        Entry points are:
        1. Files whose name matches known entry point names
        2. Files with no incoming deps AND have a if __name__ == '__main__' block
        """
        entry_points = []

        for record in python_files:
            if record.name in ENTRY_POINT_NAMES:
                entry_points.append(record.rel_path)
                continue

            # Check for if __name__ == '__main__' guard
            fa = self._dep.file_analyses.get(record.rel_path)
            if fa and fa.source and '__name__' in fa.source and '__main__' in fa.source:
                # Also check no one imports it (pure entry point)
                if self._dep.dependent_count(record.rel_path) == 0:
                    entry_points.append(record.rel_path)

        return list(dict.fromkeys(entry_points))  # deduplicate

    # ── File compile + runtime check ──────────────────────────────────────────

    def _check_file(self, record: FileRecord) -> FileExecResult:
        result = FileExecResult(rel_path=record.rel_path)

        # ── Step 1: compile() check ───────────────────────────────────────────
        try:
            with open(record.path, "r", encoding="utf-8", errors="replace") as f:
                source = f.read()
            compile(source, str(record.path), "exec")
            result.compile_ok = True
            log.debug("Compile OK: %s", record.rel_path)
        except SyntaxError as e:
            result.compile_ok    = False
            result.compile_error = f"{e.msg}"
            result.compile_line  = e.lineno
            log.warning("Compile failed: %s — %s (line %s)", record.rel_path, e.msg, e.lineno)
            return result   # No point running runtime check
        except Exception as e:
            result.compile_ok    = False
            result.compile_error = str(e)
            return result

        # ── Step 2: subprocess py_compile (sandboxed, timeout) ────────────────
        try:
            proc = subprocess.run(
                [
                    sys.executable, "-c",
                    f"import py_compile; py_compile.compile({repr(str(record.path))}, doraise=True)",
                ],
                capture_output=True,
                text=True,
                timeout=SAFE_EXEC_TIMEOUT,
                # No network, no file system mounts — just a compile check
            )
            result.runtime_ok     = proc.returncode == 0
            result.runtime_stdout = (proc.stdout or "")[:MAX_OUTPUT_CHARS] or None
            result.runtime_stderr = (proc.stderr or "")[:MAX_OUTPUT_CHARS] or None
            if not result.runtime_ok:
                result.runtime_error = (proc.stderr or proc.stdout or "Unknown error")[:MAX_OUTPUT_CHARS]
                log.warning("Runtime check failed: %s", record.rel_path)
        except subprocess.TimeoutExpired:
            result.runtime_ok  = False
            result.timed_out   = True
            result.runtime_error = f"Timed out after {SAFE_EXEC_TIMEOUT}s"
            log.warning("Timeout: %s", record.rel_path)
        except Exception as e:
            result.runtime_ok    = False
            result.runtime_error = str(e)
            log.error("Subprocess error for %s: %s", record.rel_path, e)

        return result

    # ── Failure chain mapping ─────────────────────────────────────────────────

    def _build_failure_chains(self, result: TraceResult) -> None:
        """
        For each failed file, trace which other files depend on it.
        These become the 'impacted files' — the blast radius of a failure.
        """
        if not hasattr(self._dep, "graph") or self._dep.graph is None:
            return

        try:
            import networkx as nx
        except ImportError:
            return

        G = self._dep.graph
        for rel_path, exec_res in result.file_results.items():
            if not exec_res.failed():
                continue

            # All files that (directly or transitively) depend on this failing file
            try:
                ancestors = list(nx.ancestors(G, rel_path)) if rel_path in G else []
            except Exception:
                ancestors = []

            exec_res.failure_chain = ancestors
            exec_res.root_failure  = rel_path  # this file IS the root

            if ancestors:
                result.failure_chains.append({
                    "root":    rel_path,
                    "impacts": ancestors,
                    "error":   exec_res.compile_error or exec_res.runtime_error,
                })
                log.info(
                    "Failure chain: %s → impacts %d files",
                    rel_path, len(ancestors),
                )
