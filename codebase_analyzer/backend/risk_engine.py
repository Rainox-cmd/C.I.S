"""
backend/risk_engine.py
Phase 4: Per-file risk scoring engine.
Combines dependency centrality, execution role, error frequency,
and complexity into a Low / Medium / High risk score with reasoning.

Public API:
    engine = RiskEngine(scan_result, dep_result, error_result, trace_result)
    risk_result = engine.score()   →  RiskResult
"""

from pathlib import Path
from dataclasses import dataclass, field
from collections import defaultdict

from utils.logger import get_logger
from backend.scanner import ScanResult
from backend.dependency_analyzer import DependencyResult
from backend.error_detector import ErrorResult
from backend.execution_tracer import TraceResult

log = get_logger(__name__)

# ─── Complexity scoring ────────────────────────────────────────────────────────

COMPLEXITY_KEYWORDS = {
    "Python":     ["if ", "elif ", "for ", "while ", "try:", "except", "with ", "yield"],
    "JavaScript": ["if (", "else if", "for (", "while (", "try {", "catch (", "switch ("],
    "TypeScript": ["if (", "else if", "for (", "while (", "try {", "catch (", "switch ("],
    "Java":       ["if (", "else if", "for (", "while (", "try {", "catch (", "switch ("],
    "C":          ["if (", "for (", "while (", "switch ("],
    "C++":        ["if (", "for (", "while (", "switch (", "try {"],
    "default":    ["if", "for", "while"],
}


def _complexity_score(source: str, language: str) -> int:
    keywords = COMPLEXITY_KEYWORDS.get(language, COMPLEXITY_KEYWORDS["default"])
    return sum(source.count(kw) for kw in keywords)


def _complexity_level(score: int) -> str:
    if score <= 5:   return "low"
    if score <= 15:  return "medium"
    return "high"


# ─── Risk score weights ────────────────────────────────────────────────────────

# Each factor contributes 0–1 normalized, then weighted:
W_CENTRALITY    = 0.30   # betweenness centrality (how central in graph)
W_IN_DEGREE     = 0.20   # number of files that depend on this file
W_ERRORS        = 0.25   # number and severity of issues in this file
W_EXEC_FAILURE  = 0.15   # did it fail execution checks?
W_COMPLEXITY    = 0.10   # cyclomatic-style complexity score


# ─── Data types ───────────────────────────────────────────────────────────────

@dataclass
class FileRisk:
    """Risk assessment for a single file."""
    rel_path: str
    risk_level: str          # low | medium | high
    risk_score: float        # 0.0 – 1.0 normalized
    complexity_score: int
    complexity_level: str    # low | medium | high
    lines: int
    reasoning: list[str] = field(default_factory=list)

    # Sub-scores (0.0–1.0 each)
    centrality_score: float = 0.0
    in_degree_score: float  = 0.0
    error_score: float      = 0.0
    exec_score: float       = 0.0
    complexity_factor: float= 0.0

    def to_dict(self) -> dict:
        return {
            "rel_path":          self.rel_path,
            "risk_level":        self.risk_level,
            "risk_score":        round(self.risk_score, 3),
            "complexity_score":  self.complexity_score,
            "complexity_level":  self.complexity_level,
            "lines":             self.lines,
            "reasoning":         self.reasoning,
        }


@dataclass
class RiskResult:
    """Risk scores for every file in the project."""
    file_risks: dict[str, FileRisk] = field(default_factory=dict)

    def high_risk_files(self) -> list[FileRisk]:
        return sorted(
            [r for r in self.file_risks.values() if r.risk_level == "high"],
            key=lambda r: -r.risk_score,
        )

    def medium_risk_files(self) -> list[FileRisk]:
        return [r for r in self.file_risks.values() if r.risk_level == "medium"]

    def low_risk_files(self) -> list[FileRisk]:
        return [r for r in self.file_risks.values() if r.risk_level == "low"]

    def counts(self) -> dict[str, int]:
        return {
            "high":   len(self.high_risk_files()),
            "medium": len(self.medium_risk_files()),
            "low":    len(self.low_risk_files()),
        }

    def get(self, rel_path: str) -> FileRisk | None:
        return self.file_risks.get(rel_path)

    def summary_for_ui(self) -> dict:
        """Keys match DashboardPanel.update_summary() expectations."""
        return {
            "high_risk":   len(self.high_risk_files()),
            "medium_risk": len(self.medium_risk_files()),
            "low_risk":    len(self.low_risk_files()),
        }


# ─── Engine ───────────────────────────────────────────────────────────────────

class RiskEngine:
    """
    Scores every file using a weighted multi-factor model.
    Factors: centrality, in-degree, errors, exec failure, complexity.
    """

    def __init__(
        self,
        scan_result:  ScanResult,
        dep_result:   DependencyResult,
        error_result: ErrorResult,
        trace_result: TraceResult | None = None,
    ):
        self._scan   = scan_result
        self._dep    = dep_result
        self._errors = error_result
        self._trace  = trace_result

    def score(self) -> RiskResult:
        log.info("Starting risk scoring: %d files", len(self._scan.files))
        result = RiskResult()

        # Pre-compute normalization bounds
        max_centrality = max(self._dep.centrality.values(), default=1.0) or 1.0
        max_in_degree  = max(
            (self._dep.dependent_count(f.rel_path) for f in self._scan.files),
            default=1,
        ) or 1
        max_complexity = 1  # will be updated on first pass

        # First pass: compute raw complexity scores
        raw_complexity: dict[str, int] = {}
        for record in self._scan.files:
            fa = self._dep.file_analyses.get(record.rel_path)
            source = fa.source if fa else ""
            score  = _complexity_score(source, record.language)
            raw_complexity[record.rel_path] = score
            if score > max_complexity:
                max_complexity = score

        # Error counts per file
        error_counts: dict[str, dict[str, int]] = defaultdict(lambda: {"high": 0, "medium": 0, "low": 0})
        for issue in self._errors.issues:
            sev = issue.severity
            error_counts[issue.file][sev] += 1

        max_error_weight = max(
            (c["high"] * 3 + c["medium"] * 2 + c["low"]
             for c in error_counts.values()),
            default=1,
        ) or 1

        # Execution failures
        failed_files: set[str] = set()
        if self._trace:
            failed_files = {r.rel_path for r in self._trace.failed_files()}

        # Second pass: compute risk per file
        for record in self._scan.files:
            rel_path = record.rel_path

            # ── Factor 1: Centrality ──────────────────────────────────────────
            centrality = self._dep.centrality.get(rel_path, 0.0)
            f_centrality = centrality / max_centrality

            # ── Factor 2: In-degree (how many depend on this) ─────────────────
            in_deg = self._dep.dependent_count(rel_path)
            f_in_degree = in_deg / max_in_degree

            # ── Factor 3: Error weight ────────────────────────────────────────
            ec = error_counts[rel_path]
            error_weight = ec["high"] * 3 + ec["medium"] * 2 + ec["low"]
            f_errors = error_weight / max_error_weight

            # ── Factor 4: Execution failure ───────────────────────────────────
            f_exec = 1.0 if rel_path in failed_files else 0.0

            # ── Factor 5: Complexity ──────────────────────────────────────────
            cx_score = raw_complexity.get(rel_path, 0)
            f_complexity = cx_score / max_complexity

            # ── Weighted sum ──────────────────────────────────────────────────
            risk_score = (
                W_CENTRALITY  * f_centrality  +
                W_IN_DEGREE   * f_in_degree   +
                W_ERRORS      * f_errors      +
                W_EXEC_FAILURE* f_exec        +
                W_COMPLEXITY  * f_complexity
            )

            # Clamp to [0, 1]
            risk_score = max(0.0, min(1.0, risk_score))

            # ── Risk level ────────────────────────────────────────────────────
            if risk_score >= 0.55:
                risk_level = "high"
            elif risk_score >= 0.25:
                risk_level = "medium"
            else:
                risk_level = "low"

            # ── Reasoning ────────────────────────────────────────────────────
            reasoning = self._build_reasoning(
                rel_path, f_centrality, in_deg, ec,
                rel_path in failed_files, cx_score, risk_level,
            )

            cx_level = _complexity_level(cx_score)

            file_risk = FileRisk(
                rel_path=rel_path,
                risk_level=risk_level,
                risk_score=risk_score,
                complexity_score=cx_score,
                complexity_level=cx_level,
                lines=record.lines,
                reasoning=reasoning,
                centrality_score=f_centrality,
                in_degree_score=f_in_degree,
                error_score=f_errors,
                exec_score=f_exec,
                complexity_factor=f_complexity,
            )
            result.file_risks[rel_path] = file_risk

            # Write back into ScanResult FileRecord for UI access
            record.risk = risk_level
            record.complexity_level = cx_level
            record.complexity_score = cx_score

        counts = result.counts()
        log.info(
            "Risk scoring complete: high=%d, medium=%d, low=%d",
            counts["high"], counts["medium"], counts["low"],
        )
        return result

    @staticmethod
    def _build_reasoning(
        rel_path: str,
        f_centrality: float,
        in_degree: int,
        error_counts: dict[str, int],
        exec_failed: bool,
        cx_score: int,
        risk_level: str,
    ) -> list[str]:
        reasons = []

        if f_centrality > 0.5:
            reasons.append(f"High graph centrality — critical bridge in dependency network")
        elif f_centrality > 0.2:
            reasons.append(f"Moderate centrality — several files route through this")

        if in_degree >= 5:
            reasons.append(f"{in_degree} files depend on this — failure has wide impact")
        elif in_degree >= 2:
            reasons.append(f"{in_degree} files depend on this")

        if error_counts["high"] > 0:
            reasons.append(f"{error_counts['high']} high-severity issue(s) detected")
        if error_counts["medium"] > 0:
            reasons.append(f"{error_counts['medium']} medium-severity issue(s)")

        if exec_failed:
            reasons.append("Failed execution/compile check")

        if cx_score > 20:
            reasons.append(f"High complexity score ({cx_score}) — difficult to maintain")
        elif cx_score > 10:
            reasons.append(f"Moderate complexity ({cx_score})")

        if not reasons:
            reasons.append(f"Low overall risk — {risk_level} dependency footprint")

        return reasons
