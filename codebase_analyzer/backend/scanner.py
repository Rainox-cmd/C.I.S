"""
backend/scanner.py
Phase 2: Recursive project scanner with language detection.
Refactored from the original analyzer.py — no logic changed, only modularized.

Public API:
    scanner = ProjectScanner(root_path)
    result  = scanner.scan()   →  ScanResult
"""

import os
from pathlib import Path
from dataclasses import dataclass, field
from collections import defaultdict

from utils.logger import get_logger

log = get_logger(__name__)

# ─── Constants ────────────────────────────────────────────────────────────────

IGNORE_DIRS: set[str] = {
    ".git", "node_modules", "venv", "__pycache__", ".mypy_cache",
    "dist", "build", ".tox", ".pytest_cache", ".idea", ".vscode",
    "env", ".env", "site-packages", ".next", "coverage", "htmlcov",
    # Large project additions
    ".cache", "vendor", "bower_components", "jspm_packages",
    ".sass-cache", "target", "out", "output", "generated",
    ".gradle", ".mvn", "Pods", "DerivedData",
}

LANGUAGE_MAP: dict[str, str] = {
    ".py":   "Python",
    ".js":   "JavaScript",
    ".jsx":  "JavaScript",
    ".ts":   "TypeScript",
    ".tsx":  "TypeScript",
    ".java": "Java",
    ".c":    "C",
    ".cpp":  "C++",
    ".cc":   "C++",
    ".cs":   "C#",
    ".go":   "Go",
    ".rb":   "Ruby",
    ".php":  "PHP",
    ".html": "HTML",
    ".css":  "CSS",
    ".json": "JSON",
    ".yaml": "YAML",
    ".yml":  "YAML",
    ".md":   "Markdown",
    ".sh":   "Shell",
    ".bash": "Shell",
    ".toml": "TOML",
    ".xml":  "XML",
}

FILE_CATEGORIES: dict[str, str] = {
    "source":  {".py", ".js", ".jsx", ".ts", ".tsx", ".java", ".c", ".cpp", ".cc", ".cs", ".go", ".rb", ".php"},
    "config":  {".json", ".yaml", ".yml", ".toml", ".xml", ".ini", ".cfg", ".env"},
    "assets":  {".html", ".css", ".md", ".txt", ".rst"},
}

MAX_FILE_SIZE_BYTES = 5 * 1024 * 1024  # 5 MB


# ─── Data types ───────────────────────────────────────────────────────────────

@dataclass
class FileRecord:
    """Single file entry produced by the scanner."""
    path: Path
    rel_path: str
    name: str
    ext: str
    size: int
    language: str
    category: str          # source | config | assets | other

    # Populated by later phases
    risk: str | None = None          # low | medium | high
    complexity_level: str | None = None
    complexity_score: int = 0
    lines: int = 0
    issues: list[dict] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "path":             str(self.path),
            "rel_path":         self.rel_path,
            "name":             self.name,
            "ext":              self.ext,
            "size":             self.size,
            "language":         self.language,
            "category":         self.category,
            "risk":             self.risk,
            "complexity_level": self.complexity_level,
            "complexity_score": self.complexity_score,
            "lines":            self.lines,
        }


@dataclass
class ScanResult:
    """Everything the scanner knows about the project."""
    root: Path
    files: list[FileRecord] = field(default_factory=list)
    language_counts: dict[str, int] = field(default_factory=dict)
    language_dist: dict[str, float] = field(default_factory=dict)  # percentages
    category_counts: dict[str, int] = field(default_factory=dict)
    total_size: int = 0
    scan_errors: list[str] = field(default_factory=list)

    # Convenience maps — built after scanning
    _by_rel_path: dict[str, FileRecord] = field(default_factory=dict, repr=False)
    _by_stem: dict[str, FileRecord] = field(default_factory=dict, repr=False)

    def build_maps(self) -> None:
        self._by_rel_path = {f.rel_path: f for f in self.files}
        self._by_stem = {Path(f.rel_path).stem: f for f in self.files}

    def get_by_rel_path(self, rel_path: str) -> FileRecord | None:
        return self._by_rel_path.get(rel_path)

    def get_by_stem(self, stem: str) -> FileRecord | None:
        return self._by_stem.get(stem)

    def source_files(self) -> list[FileRecord]:
        return [f for f in self.files if f.category == "source"]

    def files_for_language(self, language: str) -> list[FileRecord]:
        return [f for f in self.files if f.language == language]

    def summary_dict(self) -> dict:
        return {
            "total_files":      len(self.files),
            "languages":        self.language_counts,
            "language_dist":    self.language_dist,
            "categories":       self.category_counts,
            "total_size_bytes": self.total_size,
            "scan_errors":      len(self.scan_errors),
        }


# ─── Scanner ──────────────────────────────────────────────────────────────────

class ProjectScanner:
    """
    Recursively scans a project directory.
    Produces a ScanResult with FileRecord entries for every valid file.
    """

    def __init__(self, root_path: Path):
        self.root = root_path.resolve()

    def scan(self) -> ScanResult:
        log.info("Starting scan: %s", self.root)
        result = ScanResult(root=self.root)
        lang_counts: dict[str, int] = defaultdict(int)
        cat_counts:  dict[str, int] = defaultdict(int)

        for dirpath, dirnames, filenames in os.walk(self.root):
            # Prune ignored dirs in-place (modifies os.walk traversal)
            dirnames[:] = [
                d for d in dirnames
                if d not in IGNORE_DIRS and not d.startswith(".")
            ]

            for fname in filenames:
                fpath = Path(dirpath) / fname
                ext = fpath.suffix.lower()

                if ext not in LANGUAGE_MAP:
                    continue

                try:
                    size = fpath.stat().st_size
                except OSError as e:
                    result.scan_errors.append(f"stat failed: {fpath} — {e}")
                    continue

                if size > MAX_FILE_SIZE_BYTES:
                    log.debug("Skipping large file: %s (%d bytes)", fpath, size)
                    continue

                try:
                    rel = str(fpath.relative_to(self.root))
                except ValueError:
                    rel = str(fpath)

                lang = LANGUAGE_MAP[ext]
                cat  = self._categorize(ext)

                # Safe line counting with encoding fallback — fixes 0 lines bug
                lines = 0
                for enc in ('utf-8', 'latin-1', 'cp1252'):
                    try:
                        with open(fpath, 'r', encoding=enc) as f:
                            lines = sum(1 for _ in f)
                        break
                    except (OSError, UnicodeDecodeError):
                        continue

                record = FileRecord(
                    path=fpath,
                    rel_path=rel,
                    name=fname,
                    ext=ext,
                    size=size,
                    language=lang,
                    category=cat,
                    lines=lines,
                )
                result.files.append(record)
                result.total_size += size
                lang_counts[lang] += 1
                cat_counts[cat] += 1

        # Build distributions
        total = len(result.files) or 1
        result.language_counts = dict(lang_counts)
        result.language_dist = {
            lang: round(count / total * 100, 1)
            for lang, count in lang_counts.items()
        }
        result.category_counts = dict(cat_counts)
        result.build_maps()

        log.info(
            "Scan complete: %d files, %d languages, %d errors",
            len(result.files), len(lang_counts), len(result.scan_errors),
        )
        return result

    @staticmethod
    def _categorize(ext: str) -> str:
        for cat, exts in FILE_CATEGORIES.items():
            if ext in exts:
                return cat
        return "other"
