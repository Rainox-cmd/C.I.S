"""
ai/context_builder.py
Phase 6: Context builder for AI queries.

Given a user question and all analysis results, selects ONLY the relevant
files, dependencies, errors, and risks — never the full codebase.
Keeps total token count under MAX_INPUT_TOKENS.

Public API:
    builder = ContextBuilder(scan, dep, errors, trace, risk)
    context = builder.build(question, focus_file=None)  -> ProjectContext
"""

import re
from dataclasses import dataclass, field
from pathlib import Path

from utils.logger import get_logger
from backend.scanner import ScanResult
from backend.dependency_analyzer import DependencyResult
from backend.error_detector import ErrorResult
from backend.execution_tracer import TraceResult
from backend.risk_engine import RiskResult

log = get_logger(__name__)

# ─── Token budget ─────────────────────────────────────────────────────────────

MAX_INPUT_TOKENS  = 6000    # hard ceiling for full context
CHARS_PER_TOKEN   = 4       # rough estimate: 1 token ≈ 4 chars
MAX_INPUT_CHARS   = MAX_INPUT_TOKENS * CHARS_PER_TOKEN  # 24 000 chars
MAX_FILE_SNIPPET  = 800     # max chars of source code per file in context
MAX_FILES_IN_CTX  = 8       # max number of files included


# ─── Data types ───────────────────────────────────────────────────────────────

@dataclass
class FileContext:
    """Context for a single file included in the AI prompt."""
    rel_path:    str
    language:    str
    risk_level:  str
    lines:       int
    snippet:     str          # truncated source (most important part)
    issues:      list[str]    # issue messages for this file
    imports:     list[str]    # what this file imports
    imported_by: list[str]    # what imports this file
    is_entry:    bool
    reasoning:   list[str]    # risk reasoning


@dataclass
class ProjectContext:
    """
    The full structured context sent to the AI.
    Serialised by PromptEngine into the prompt template.
    """
    question:        str
    focus_file:      str | None
    project_root:    str

    # Summary stats
    total_files:     int
    language_dist:   dict[str, float]
    total_issues:    int
    circular_deps:   int
    entry_points:    list[str]
    most_central:    str | None

    # Selected files
    files:           list[FileContext] = field(default_factory=list)

    # High-level issues (not file-specific)
    global_issues:   list[str] = field(default_factory=list)

    # Failure chains from execution tracer
    failure_chains:  list[dict] = field(default_factory=list)

    # Top risk files summary
    high_risk_files: list[str] = field(default_factory=list)

    # Estimated token usage
    estimated_tokens: int = 0

    def is_empty(self) -> bool:
        return not self.files and not self.global_issues


# ─── Keyword matchers ─────────────────────────────────────────────────────────

# Maps question keywords → what kind of context to prioritise
_INTENT_PATTERNS = {
    "error":        re.compile(r'\b(error|fail|crash|bug|broken|syntax|exception)\b', re.I),
    "risk":         re.compile(r'\b(risk|dangerous|unsafe|critical|important|central)\b', re.I),
    "dependency":   re.compile(r'\b(depend|import|require|circular|cycle|link)\b', re.I),
    "entry":        re.compile(r'\b(entry|start|main|run|execute|launch)\b', re.I),
    "complexity":   re.compile(r'\b(complex|complicated|hard|maintainab|refactor)\b', re.I),
    "unused":       re.compile(r'\b(unused|dead|orphan|unreachable|never called)\b', re.I),
}


def _detect_intent(question: str) -> set[str]:
    intents = set()
    for intent, pattern in _INTENT_PATTERNS.items():
        if pattern.search(question):
            intents.add(intent)
    return intents or {"general"}


def _extract_mentioned_files(question: str, scan: ScanResult) -> list[str]:
    """Find any file names explicitly mentioned in the question."""
    mentioned = []
    q_lower = question.lower()
    for record in scan.files:
        if record.name.lower() in q_lower or record.rel_path.lower() in q_lower:
            mentioned.append(record.rel_path)
    return mentioned


# ─── Builder ─────────────────────────────────────────────────────────────────

class ContextBuilder:
    """
    Builds a focused ProjectContext for a given question.
    Strategy:
      1. Detect question intent
      2. Score every file for relevance to the question
      3. Select top-N files within token budget
      4. Include snippets (truncated to MAX_FILE_SNIPPET chars)
      5. Add global issues and failure chains
    """

    def __init__(
        self,
        scan:   ScanResult,
        dep:    DependencyResult,
        errors: ErrorResult,
        trace:  TraceResult,
        risk:   RiskResult,
    ):
        self._scan   = scan
        self._dep    = dep
        self._errors = errors
        self._trace  = trace
        self._risk   = risk

    # ── Public ────────────────────────────────────────────────────────────────

    def build(self, question: str, focus_file: str | None = None) -> ProjectContext:
        """
        Build context for a question.
        focus_file: if the user clicked a specific file before asking,
                    that file is always included and scored highest.
        """
        log.info("Building context: question=%r focus=%s", question[:60], focus_file)

        intents       = _detect_intent(question)
        mentioned     = _extract_mentioned_files(question, self._scan)
        selected_files = self._select_files(question, intents, mentioned, focus_file)
        file_contexts  = [self._build_file_ctx(r) for r in selected_files]
        global_issues  = self._collect_global_issues()
        chains         = self._collect_failure_chains()
        high_risk      = [r.rel_path for r in self._risk.high_risk_files()[:5]]

        ctx = ProjectContext(
            question       = question,
            focus_file     = focus_file,
            project_root   = str(self._scan.root),
            total_files    = len(self._scan.files),
            language_dist  = self._scan.language_dist,
            total_issues   = len(self._errors.issues),
            circular_deps  = len(self._dep.circular_deps),
            entry_points   = self._dep.entry_points[:5],
            most_central   = self._dep.most_central,
            files          = file_contexts,
            global_issues  = global_issues,
            failure_chains = chains,
            high_risk_files= high_risk,
        )

        # Estimate token usage
        ctx.estimated_tokens = self._estimate_tokens(ctx)
        log.info(
            "Context built: %d files, ~%d tokens, intents=%s",
            len(file_contexts), ctx.estimated_tokens, intents,
        )
        return ctx

    # ── File selection ────────────────────────────────────────────────────────

    def _select_files(
        self,
        question:    str,
        intents:     set[str],
        mentioned:   list[str],
        focus_file:  str | None,
    ) -> list:
        """Score every file and return the top-N most relevant ones."""

        scores: dict[str, float] = {}

        for record in self._scan.files:
            p = record.rel_path
            score = 0.0

            # Always include: explicitly mentioned in question
            if p in mentioned:
                score += 20.0

            # Always include: user's focus file
            if p == focus_file:
                score += 25.0

            # Intent-based scoring
            risk_rec = self._risk.get(p)
            risk_score = risk_rec.risk_score if risk_rec else 0.0

            if "error" in intents:
                issue_count = len(self._errors.by_file(p))
                score += issue_count * 5.0
                if any(r.rel_path == p for r in self._trace.failed_files()):
                    score += 8.0

            if "risk" in intents:
                score += risk_score * 15.0

            if "dependency" in intents:
                in_cycle = self._dep.is_in_cycle(p)
                score += (10.0 if in_cycle else 0.0)
                score += self._dep.dependent_count(p) * 0.5
                score += self._dep.dependency_count(p) * 0.3

            if "entry" in intents:
                if p in self._dep.entry_points:
                    score += 12.0

            if "complexity" in intents:
                if risk_rec:
                    score += risk_rec.complexity_score * 0.2

            if "unused" in intents:
                unused = [
                    i for i in self._errors.by_file(p)
                    if i.issue_type in ("unused_function", "isolated_file")
                ]
                score += len(unused) * 4.0

            # General: always include high-risk and central files
            score += risk_score * 3.0
            score += self._dep.centrality.get(p, 0.0) * 5.0

            scores[p] = score

        # Sort by score desc, take top MAX_FILES_IN_CTX
        ranked = sorted(scores.items(), key=lambda x: -x[1])
        top_paths = [p for p, _ in ranked[:MAX_FILES_IN_CTX] if scores[p] > 0]

        # Ensure focus_file and mentioned files are always included
        must_include = list(dict.fromkeys(
            ([focus_file] if focus_file else []) + mentioned
        ))
        for p in must_include:
            if p not in top_paths and self._scan.get_by_rel_path(p):
                top_paths.insert(0, p)
        top_paths = top_paths[:MAX_FILES_IN_CTX]

        return [self._scan.get_by_rel_path(p) for p in top_paths if self._scan.get_by_rel_path(p)]

    # ── File context construction ──────────────────────────────────────────────

    def _build_file_ctx(self, record) -> FileContext:
        fa       = self._dep.file_analyses.get(record.rel_path)
        risk_rec = self._risk.get(record.rel_path)
        issues   = [i.message for i in self._errors.by_file(record.rel_path)]
        imports  = list(dict.fromkeys(fa.imports[:12] if fa else []))
        imported_by = self._dep.get_dependents(record.rel_path)[:6]

        # Build snippet: first MAX_FILE_SNIPPET chars of source
        snippet = ""
        if fa and fa.source:
            raw = fa.source.strip()
            if len(raw) > MAX_FILE_SNIPPET:
                # Keep the beginning (imports + first functions)
                snippet = raw[:MAX_FILE_SNIPPET] + "\n… (truncated)"
            else:
                snippet = raw

        return FileContext(
            rel_path    = record.rel_path,
            language    = record.language,
            risk_level  = risk_rec.risk_level if risk_rec else "low",
            lines       = record.lines,
            snippet     = snippet,
            issues      = issues[:6],
            imports     = imports,
            imported_by = imported_by,
            is_entry    = record.rel_path in self._dep.entry_points,
            reasoning   = risk_rec.reasoning[:3] if risk_rec else [],
        )

    # ── Global issues ─────────────────────────────────────────────────────────

    def _collect_global_issues(self) -> list[str]:
        """High-level issues not tied to a single file."""
        global_issues = []

        # Circular dependencies
        for cycle in self._dep.circular_deps[:3]:
            chain = " → ".join(cycle) + f" → {cycle[0]}"
            global_issues.append(f"Circular dependency: {chain}")

        # Failure chains
        for chain in self._trace.failure_chains[:3]:
            root = chain["root"]
            n    = len(chain["impacts"])
            global_issues.append(
                f"Execution failure in '{root}' impacts {n} downstream file(s)"
            )

        return global_issues

    def _collect_failure_chains(self) -> list[dict]:
        return self._trace.failure_chains[:4]

    # ── Token estimation ──────────────────────────────────────────────────────

    @staticmethod
    def _estimate_tokens(ctx: ProjectContext) -> int:
        """Rough token count: total chars / CHARS_PER_TOKEN."""
        total_chars = len(ctx.question)
        for fc in ctx.files:
            total_chars += len(fc.rel_path) + len(fc.snippet)
            total_chars += sum(len(i) for i in fc.issues)
        total_chars += sum(len(i) for i in ctx.global_issues)
        total_chars += 500   # prompt template overhead
        return total_chars // CHARS_PER_TOKEN
