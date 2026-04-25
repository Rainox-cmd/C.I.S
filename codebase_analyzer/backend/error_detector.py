"""
backend/error_detector.py
Phase 2: Static error detection across all analyzed files.
Collects syntax errors, unused functions, and import issues
from DependencyResult and ScanResult.

Public API:
    detector = ErrorDetector(scan_result, dep_result)
    error_result = detector.detect()   →  ErrorResult
"""

from pathlib import Path
from dataclasses import dataclass, field

from utils.logger import get_logger
from backend.scanner import ScanResult
from backend.dependency_analyzer import DependencyResult

log = get_logger(__name__)


# ─── Data types ───────────────────────────────────────────────────────────────

@dataclass
class Issue:
    """A single detected code issue."""
    severity: str          # high | medium | low
    issue_type: str        # syntax_error | circular_dep | unused_function | ...
    file: str              # rel_path
    message: str
    line: int | None = None
    details: dict = field(default_factory=dict)

    def to_dict(self) -> dict:
        return {
            "severity":   self.severity,
            "issue_type": self.issue_type,
            "file":       self.file,
            "message":    self.message,
            "line":       self.line,
        }

    def to_console_dict(self) -> dict:
        """Format for ErrorConsolePanel.load_errors()."""
        SEV_MAP = {"high": "ERROR", "medium": "WARNING", "low": "INFO"}
        return {
            "severity": SEV_MAP.get(self.severity, "INFO"),
            "file":     self.file,
            "message":  self.message,
            "line":     self.line,
        }


@dataclass
class ErrorResult:
    """All issues detected across the project."""
    issues: list[Issue] = field(default_factory=list)

    def by_file(self, rel_path: str) -> list[Issue]:
        return [i for i in self.issues if i.file == rel_path]

    def by_severity(self, severity: str) -> list[Issue]:
        return [i for i in self.issues if i.severity == severity]

    def by_type(self, issue_type: str) -> list[Issue]:
        return [i for i in self.issues if i.issue_type == issue_type]

    def count_by_severity(self) -> dict[str, int]:
        counts = {"high": 0, "medium": 0, "low": 0}
        for i in self.issues:
            counts[i.severity] = counts.get(i.severity, 0) + 1
        return counts

    def for_console(self) -> list[dict]:
        """Ready-to-use list for ErrorConsolePanel.load_errors()."""
        return [i.to_console_dict() for i in self.issues]


# ─── Detector ─────────────────────────────────────────────────────────────────

class ErrorDetector:
    """
    Runs static analysis checks on the parsed project data.
    Does NOT execute any code — that is handled by ExecutionTracer in Phase 3.
    """

    # Known third-party / stdlib packages — skip "missing import" for these
    _KNOWN_PACKAGES = {
        # Python stdlib samples
        "os", "sys", "re", "ast", "json", "math", "time", "datetime",
        "pathlib", "collections", "itertools", "functools", "typing",
        "subprocess", "threading", "asyncio", "logging", "argparse",
        "hashlib", "zipfile", "tempfile", "shutil", "copy", "io",
        "dataclasses", "abc", "enum", "contextlib", "inspect",
        # Common third-party
        "numpy", "pandas", "requests", "flask", "django", "fastapi",
        "sqlalchemy", "pytest", "networkx", "matplotlib", "PIL",
        "scipy", "sklearn", "tensorflow", "torch", "textual", "rich",
        "openai", "pydantic", "uvicorn", "aiohttp", "httpx",
        # JS/Node builtins
        "react", "vue", "angular", "express", "lodash", "axios",
        "path", "fs", "http", "https", "url", "events", "stream",
        # Java stdlib
        "java", "javax", "org", "com", "sun",
    }

    # Functions that are always "used" even if not called explicitly
    _ALWAYS_USED_FUNCS = {
        "main", "__init__", "__str__", "__repr__", "__len__", "__eq__",
        "__hash__", "__call__", "__enter__", "__exit__", "__iter__",
        "setUp", "tearDown", "render", "get", "post", "put", "delete",
        "handle", "run", "start", "stop", "connect", "disconnect",
    }

    def __init__(self, scan_result: ScanResult, dep_result: DependencyResult):
        self._scan = scan_result
        self._dep  = dep_result

    def detect(self) -> ErrorResult:
        log.info("Starting error detection")
        result = ErrorResult()

        self._detect_syntax_errors(result)
        self._detect_circular_deps(result)
        self._detect_isolated_files(result)
        self._detect_excessive_deps(result)
        self._detect_unused_functions(result)
        self._detect_missing_imports(result)

        counts = result.count_by_severity()
        log.info(
            "Error detection complete: %d issues (high=%d, medium=%d, low=%d)",
            len(result.issues), counts["high"], counts["medium"], counts["low"],
        )
        return result

    # ── Checks ────────────────────────────────────────────────────────────────

    def _detect_syntax_errors(self, result: ErrorResult) -> None:
        for rel_path, fa in self._dep.file_analyses.items():
            if not fa.syntax_ok and fa.syntax_error:
                # Extract line number if present
                line = None
                if fa.syntax_error.startswith("Line "):
                    try:
                        line = int(fa.syntax_error.split(":")[0].replace("Line ", ""))
                    except ValueError:
                        pass
                result.issues.append(Issue(
                    severity="high",
                    issue_type="syntax_error",
                    file=rel_path,
                    message=f"Syntax error — {fa.syntax_error}",
                    line=line,
                ))
                log.warning("Syntax error in %s: %s", rel_path, fa.syntax_error)

    def _detect_circular_deps(self, result: ErrorResult) -> None:
        for cycle in self._dep.circular_deps:
            cycle_str = " → ".join(cycle) + f" → {cycle[0]}"
            # Flag each file in the cycle
            for file in cycle:
                result.issues.append(Issue(
                    severity="high",
                    issue_type="circular_dependency",
                    file=file,
                    message=f"Circular dependency: {cycle_str}",
                    details={"cycle": cycle},
                ))

    def _detect_isolated_files(self, result: ErrorResult) -> None:
        ANALYSIS_LANGS = {"Python", "JavaScript", "TypeScript", "Java", "C", "C++"}
        for rel_path in self._dep.dead_files:
            record = self._scan.get_by_rel_path(rel_path)
            if not record or record.language not in ANALYSIS_LANGS:
                continue
            result.issues.append(Issue(
                severity="low",
                issue_type="isolated_file",
                file=rel_path,
                message="File has no imports and is not imported anywhere (dead file)",
            ))

    def _detect_excessive_deps(self, result: ErrorResult) -> None:
        THRESHOLD = 10
        for rel_path, fa in self._dep.file_analyses.items():
            dep_count = len(fa.local_deps)
            if dep_count >= THRESHOLD:
                result.issues.append(Issue(
                    severity="medium",
                    issue_type="excessive_dependencies",
                    file=rel_path,
                    message=f"File has {dep_count} local dependencies (threshold: {THRESHOLD})",
                ))

    def _detect_unused_functions(self, result: ErrorResult) -> None:
        """
        Check if a function defined in one file is called anywhere else.
        Limitation: string-search based, not semantic. False positives possible.
        """
        # Build corpus of all source (excluding the defining file)
        sources: dict[str, str] = {}
        for rel_path, fa in self._dep.file_analyses.items():
            sources[rel_path] = fa.source

        for rel_path, fa in self._dep.file_analyses.items():
            other_corpus = "\n".join(
                src for path, src in sources.items() if path != rel_path
            )
            for func in fa.functions:
                if func.startswith("_"):
                    continue   # skip private/dunder
                if func in self._ALWAYS_USED_FUNCS:
                    continue
                if len(func) <= 2:
                    continue   # too short to be meaningful
                # Count occurrences in other files
                if other_corpus.count(func) == 0:
                    result.issues.append(Issue(
                        severity="low",
                        issue_type="unused_function",
                        file=rel_path,
                        message=f"Function '{func}' appears unused outside its own file",
                    ))

    def _detect_missing_imports(self, result: ErrorResult) -> None:
        """
        Flag imports that cannot be found in stdlib/known packages
        AND cannot be resolved to a local file.
        Heuristic — will have false positives for exotic packages.
        """
        for rel_path, fa in self._dep.file_analyses.items():
            record = self._scan.get_by_rel_path(rel_path)
            if not record or record.language not in {"Python"}:
                continue  # Only Python for now (AST gives reliable imports)

            for imp in fa.imports:
                if imp in self._KNOWN_PACKAGES:
                    continue
                # Check if it resolves to a local file
                local = self._scan.get_by_stem(imp)
                if local:
                    continue
                result.issues.append(Issue(
                    severity="low",
                    issue_type="missing_import",
                    file=rel_path,
                    message=f"Import '{imp}' not found locally or in known packages",
                ))
