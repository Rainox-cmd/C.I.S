"""
backend/dependency_analyzer.py
Phase 2: Extracts imports from each file and builds a NetworkX dependency graph.
Supports Python (AST), JavaScript/TypeScript (regex), Java (regex).

Public API:
    analyzer = DependencyAnalyzer(scan_result)
    dep_result = analyzer.analyze()   →  DependencyResult
"""

import ast
import re
from pathlib import Path
from dataclasses import dataclass, field
from collections import defaultdict

from utils.logger import get_logger
from backend.scanner import ScanResult, FileRecord

log = get_logger(__name__)

try:
    import networkx as nx
    HAS_NETWORKX = True
except ImportError:
    HAS_NETWORKX = False
    log.warning("networkx not installed — graph features disabled. pip install networkx")


# ─── Regex patterns (JS/TS/Java) ──────────────────────────────────────────────

_JS_IMPORT_PATTERNS = [
    re.compile(r'''import\s+(?:.*?\s+from\s+)?['"]([^'"]+)['"]'''),
    re.compile(r'''require\s*\(\s*['"]([^'"]+)['"]\s*\)'''),
    re.compile(r'''import\s*\(\s*['"]([^'"]+)['"]\s*\)'''),  # dynamic import
]

_JS_FUNC_PATTERN = re.compile(
    r'''(?:function\s+(\w+)|(?:const|let|var)\s+(\w+)\s*=\s*(?:async\s*)?\(|'''
    r'''(?:async\s+)?function\s*\*?\s*(\w+)\s*\()'''
)

_JAVA_IMPORT_PATTERN = re.compile(r'^import\s+([\w.]+)\s*;', re.MULTILINE)
_JAVA_METHOD_PATTERN = re.compile(
    r'(?:public|private|protected|static|\s)+[\w<>\[\]]+\s+(\w+)\s*\('
)


# ─── Data types ───────────────────────────────────────────────────────────────

@dataclass
class FileAnalysis:
    """Per-file analysis result: imports, functions, syntax status."""
    rel_path: str
    imports: list[str] = field(default_factory=list)       # raw import names
    local_deps: list[str] = field(default_factory=list)    # resolved rel_paths
    functions: list[str] = field(default_factory=list)
    classes: list[str] = field(default_factory=list)
    syntax_ok: bool = True
    syntax_error: str | None = None
    source: str = ""                                        # raw source (for later phases)


@dataclass
class DependencyResult:
    """Full dependency analysis for a project."""
    file_analyses: dict[str, FileAnalysis] = field(default_factory=dict)
    graph: object = None           # nx.DiGraph or None
    circular_deps: list[list[str]] = field(default_factory=list)
    entry_points: list[str] = field(default_factory=list)
    dead_files: list[str] = field(default_factory=list)
    most_central: str | None = None
    centrality: dict[str, float] = field(default_factory=dict)
    edges: list[dict] = field(default_factory=list)        # [{source, target}]

    def dependency_count(self, rel_path: str) -> int:
        """How many files does this file import."""
        return len(self.file_analyses.get(rel_path, FileAnalysis(rel_path)).local_deps)

    def dependent_count(self, rel_path: str) -> int:
        """How many files import this file (in-degree)."""
        if not HAS_NETWORKX or self.graph is None:
            return 0
        return self.graph.in_degree(rel_path) if rel_path in self.graph else 0

    def get_dependents(self, rel_path: str) -> list[str]:
        """Files that directly import rel_path."""
        if not HAS_NETWORKX or self.graph is None:
            return []
        return list(self.graph.predecessors(rel_path)) if rel_path in self.graph else []

    def get_dependencies(self, rel_path: str) -> list[str]:
        """Files that rel_path directly imports."""
        if not HAS_NETWORKX or self.graph is None:
            return []
        return list(self.graph.successors(rel_path)) if rel_path in self.graph else []

    def is_in_cycle(self, rel_path: str) -> bool:
        for cycle in self.circular_deps:
            if rel_path in cycle:
                return True
        return False


# ─── Language-specific analyzers ──────────────────────────────────────────────

class _PythonParser:
    def parse(self, record: FileRecord) -> FileAnalysis:
        fa = FileAnalysis(rel_path=record.rel_path)
        try:
            with open(record.path, "r", encoding="utf-8", errors="replace") as f:
                fa.source = f.read()
            tree = ast.parse(fa.source, filename=str(record.path))
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    for alias in node.names:
                        fa.imports.append(alias.name.split(".")[0])
                elif isinstance(node, ast.ImportFrom):
                    if node.module:
                        fa.imports.append(node.module.split(".")[0])
                elif isinstance(node, ast.FunctionDef):
                    fa.functions.append(node.name)
                elif isinstance(node, ast.AsyncFunctionDef):
                    fa.functions.append(node.name)
                elif isinstance(node, ast.ClassDef):
                    fa.classes.append(node.name)
        except SyntaxError as e:
            fa.syntax_ok = False
            fa.syntax_error = f"Line {e.lineno}: {e.msg}"
        except Exception as e:
            fa.syntax_ok = False
            fa.syntax_error = str(e)
        return fa


class _JavaScriptParser:
    def parse(self, record: FileRecord) -> FileAnalysis:
        fa = FileAnalysis(rel_path=record.rel_path)
        try:
            with open(record.path, "r", encoding="utf-8", errors="replace") as f:
                fa.source = f.read()
            for pat in _JS_IMPORT_PATTERNS:
                for m in pat.finditer(fa.source):
                    imp = m.group(1)
                    if not imp.startswith("."):
                        fa.imports.append(imp.split("/")[0].lstrip("@"))
                    else:
                        # Relative import — keep as-is for local resolution
                        fa.imports.append(imp)
            for m in _JS_FUNC_PATTERN.finditer(fa.source):
                name = m.group(1) or m.group(2) or m.group(3)
                if name:
                    fa.functions.append(name)
        except Exception as e:
            fa.syntax_ok = False
            fa.syntax_error = str(e)
        return fa


class _JavaParser:
    def parse(self, record: FileRecord) -> FileAnalysis:
        fa = FileAnalysis(rel_path=record.rel_path)
        try:
            with open(record.path, "r", encoding="utf-8", errors="replace") as f:
                fa.source = f.read()
            for m in _JAVA_IMPORT_PATTERN.finditer(fa.source):
                pkg = m.group(1).split(".")[0]
                fa.imports.append(pkg)
            for m in _JAVA_METHOD_PATTERN.finditer(fa.source):
                fa.functions.append(m.group(1))
        except Exception as e:
            fa.syntax_ok = False
            fa.syntax_error = str(e)
        return fa


class _GenericParser:
    def parse(self, record: FileRecord) -> FileAnalysis:
        fa = FileAnalysis(rel_path=record.rel_path)
        try:
            with open(record.path, "r", encoding="utf-8", errors="replace") as f:
                fa.source = f.read()
        except Exception:
            pass
        return fa


_PARSERS = {
    "Python":     _PythonParser(),
    "JavaScript": _JavaScriptParser(),
    "TypeScript": _JavaScriptParser(),
    "Java":       _JavaParser(),
}
_GENERIC = _GenericParser()


# ─── Main analyzer ────────────────────────────────────────────────────────────

class DependencyAnalyzer:
    """
    Analyzes all source files and builds a dependency graph.
    Input: ScanResult from ProjectScanner.
    Output: DependencyResult.
    """

    def __init__(self, scan_result: ScanResult):
        self._scan = scan_result

    def analyze(self) -> DependencyResult:
        log.info("Starting dependency analysis: %d files", len(self._scan.files))
        result = DependencyResult()

        # Step 1: parse each file
        for record in self._scan.files:
            parser = _PARSERS.get(record.language, _GENERIC)
            fa = parser.parse(record)
            result.file_analyses[record.rel_path] = fa
            log.debug("Parsed %s: %d imports, %d funcs, syntax_ok=%s",
                      record.rel_path, len(fa.imports), len(fa.functions), fa.syntax_ok)

        # Step 2: resolve local dependencies
        self._resolve_local_deps(result)

        # Step 3: build graph
        if HAS_NETWORKX:
            self._build_graph(result)
            self._detect_circular_deps(result)
            self._compute_centrality(result)
            self._find_entry_and_dead(result)

        log.info(
            "Dependency analysis complete: %d edges, %d circular deps, %d entry points",
            len(result.edges), len(result.circular_deps), len(result.entry_points),
        )
        return result

    # ── Local dependency resolution ───────────────────────────────────────────

    def _resolve_local_deps(self, result: DependencyResult) -> None:
        """
        Match each import name against known project file stems.
        Sets FileAnalysis.local_deps to list of resolved rel_paths.
        """
        stem_map = self._scan._by_stem   # stem → FileRecord

        for rel_path, fa in result.file_analyses.items():
            source_record = self._scan.get_by_rel_path(rel_path)
            resolved = []
            for imp in fa.imports:
                # Direct stem match
                target = stem_map.get(imp)
                if target and target.rel_path != rel_path:
                    resolved.append(target.rel_path)
                    continue
                # Relative path match (JS: "./utils/helpers")
                if imp.startswith(".") and source_record:
                    candidate = self._resolve_relative(source_record, imp)
                    if candidate:
                        resolved.append(candidate)
            fa.local_deps = list(dict.fromkeys(resolved))  # deduplicate, preserve order

    def _resolve_relative(self, source: FileRecord, rel_import: str) -> str | None:
        """Resolve a relative JS/TS import to a known file rel_path."""
        source_dir = Path(source.rel_path).parent
        candidate_base = (source_dir / rel_import).resolve()

        for ext in (".js", ".jsx", ".ts", ".tsx", ".py"):
            candidate = Path(source.path).parent / (rel_import.lstrip("./") + ext)
            # Try by stem in scan map
            stem = Path(rel_import).stem
            record = self._scan.get_by_stem(stem)
            if record and record.rel_path != source.rel_path:
                return record.rel_path
        return None

    # ── Graph construction ────────────────────────────────────────────────────

    def _build_graph(self, result: DependencyResult) -> None:
        G = nx.DiGraph()

        # Add all files as nodes
        for record in self._scan.files:
            G.add_node(record.rel_path, language=record.language, size=record.size)

        # Add edges from local deps
        for rel_path, fa in result.file_analyses.items():
            for dep in fa.local_deps:
                if dep in G:
                    G.add_edge(rel_path, dep)
                    result.edges.append({"source": rel_path, "target": dep})

        result.graph = G
        log.debug("Graph: %d nodes, %d edges", G.number_of_nodes(), G.number_of_edges())

    def _detect_circular_deps(self, result: DependencyResult) -> None:
        try:
            cycles = list(nx.simple_cycles(result.graph))
            result.circular_deps = cycles
            if cycles:
                log.warning("Circular dependencies found: %d cycles", len(cycles))
        except Exception as e:
            log.error("Cycle detection failed: %s", e)

    def _compute_centrality(self, result: DependencyResult) -> None:
        try:
            centrality = nx.betweenness_centrality(result.graph)
            result.centrality = centrality
            if centrality:
                result.most_central = max(centrality, key=centrality.get)
        except Exception as e:
            log.warning("Centrality computation failed: %s", e)

    def _find_entry_and_dead(self, result: DependencyResult) -> None:
        G = result.graph
        ENTRY_NAMES = {"main", "app", "index", "server", "run", "start", "manage"}

        for node in G.nodes():
            stem = Path(node).stem.lower()
            # Entry point: no one imports it AND it imports others
            if G.in_degree(node) == 0 and G.out_degree(node) > 0:
                result.entry_points.append(node)
            # Dead file: no imports at all and not imported by anyone
            elif G.in_degree(node) == 0 and G.out_degree(node) == 0:
                result.dead_files.append(node)
            # Also flag by well-known entry names
            elif stem in ENTRY_NAMES and node not in result.entry_points:
                result.entry_points.append(node)
