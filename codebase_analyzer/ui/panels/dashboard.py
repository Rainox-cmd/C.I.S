"""
ui/panels/dashboard.py
Center-top panel: analysis summary, language breakdown, key metrics.
Phase 1 — Layout and placeholder data only.
"""

from textual.app import ComposeResult
from textual.widget import Widget
from textual.widgets import Static, DataTable, Label
from textual.containers import Vertical, Horizontal, ScrollableContainer
from rich.text import Text
from rich.table import Table
from rich.panel import Panel
from rich.columns import Columns
from rich import box


# ─── Risk color helpers ────────────────────────────────────────────────────────

def risk_text(level: str) -> Text:
    colors = {"high": "bold red", "medium": "bold yellow", "low": "bold green"}
    return Text(level.upper(), style=colors.get(level, "dim"))


def severity_badge(count: int, level: str) -> Text:
    if count == 0:
        return Text("0", style="dim")
    colors = {"high": "bold red", "medium": "bold yellow", "low": "dim green"}
    return Text(str(count), style=colors.get(level, "white"))


# ─── Metric card (Rich renderable) ────────────────────────────────────────────

def _metric_card(title: str, value: str, style: str = "white") -> Panel:
    return Panel(
        Text(value, style=f"bold {style}", justify="center"),
        title=f"[dim]{title}[/dim]",
        border_style="bright_black",
        box=box.ROUNDED,
        padding=(0, 1),
    )


class DashboardPanel(Widget):
    """
    Top-center panel: project summary metrics and language distribution.

    Phase 1 responsibilities:
    - Render metric cards (total files, issues, languages, etc.)
    - Render language distribution bar
    - Render issues summary table
    - All values are placeholder — Phase 2 wires real data

    Public API:
    - update_summary(data: dict) → refresh all metrics
    """

    DEFAULT_CSS = """
    DashboardPanel {
        height: 100%;
        background: $panel;
        padding: 0;
    }

    #dash-title {
        background: $primary-darken-2;
        color: $text;
        padding: 0 1;
        text-style: bold;
        width: 100%;
    }

    #metrics-row {
        height: 5;
        margin: 1 1 0 1;
    }

    #metric-files {
        width: 1fr;
    }
    #metric-issues {
        width: 1fr;
    }
    #metric-langs {
        width: 1fr;
    }
    #metric-risk {
        width: 1fr;
    }
    #metric-time {
        width: 1fr;
    }

    #lang-bar-container {
        margin: 0 1;
        height: 4;
    }

    #issues-table-container {
        margin: 0 1;
        height: 1fr;
        overflow-y: auto;
    }

    #dash-status {
        height: 1;
        padding: 0 1;
        color: $text-muted;
        background: $panel-darken-1;
    }
    """

    # Placeholder data shown before any project is loaded
    _PLACEHOLDER = {
        "total_files": 0,
        "total_issues": 0,
        "languages": {},
        "high_risk": 0,
        "medium_risk": 0,
        "low_risk": 0,
        "circular_deps": 0,
        "syntax_errors": 0,
        "entry_points": [],
        "most_central": None,
        "analysis_time": None,
    }

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._data = dict(self._PLACEHOLDER)

    def compose(self) -> ComposeResult:
        yield Static(" 📊 ANALYSIS DASHBOARD", id="dash-title")

        with Horizontal(id="metrics-row"):
            yield Static(id="metric-files")
            yield Static(id="metric-issues")
            yield Static(id="metric-langs")
            yield Static(id="metric-risk")
            yield Static(id="metric-time")

        yield Static(id="lang-bar-container")

        with ScrollableContainer(id="issues-table-container"):
            yield Static(id="issues-table")

        yield Static("Awaiting analysis…", id="dash-status")

    def on_mount(self) -> None:
        self._render_all()

    # ── Public API ────────────────────────────────────────────────────────────

    def update_summary(self, data: dict) -> None:
        """
        Called by main_window after scan completes.
        data keys mirror _PLACEHOLDER above.
        """
        self._data.update(data)
        self._render_all()
        self.query_one("#dash-status", Static).update(
            f" Last analysis: {data.get('analysis_time', 'N/A')}s"
        )

    def set_status(self, message: str) -> None:
        self.query_one("#dash-status", Static).update(f" {message}")

    # ── Rendering ────────────────────────────────────────────────────────────

    def _render_all(self) -> None:
        d = self._data
        self._render_metrics(d)
        self._render_lang_bar(d)
        self._render_issues_table(d)

    def _render_metrics(self, d: dict) -> None:
        self.query_one("#metric-files", Static).update(
            _metric_card("Total Files", str(d["total_files"]), "cyan")
        )
        issue_color = "red" if d["total_issues"] > 0 else "green"
        self.query_one("#metric-issues", Static).update(
            _metric_card("Issues", str(d["total_issues"]), issue_color)
        )
        self.query_one("#metric-langs", Static).update(
            _metric_card("Languages", str(len(d["languages"])), "yellow")
        )
        risk_color = "red" if d["high_risk"] > 0 else "yellow" if d["medium_risk"] > 0 else "green"
        self.query_one("#metric-risk", Static).update(
            _metric_card("High Risk", str(d["high_risk"]), risk_color)
        )
        time_val = f"{d['analysis_time']}s" if d["analysis_time"] else "—"
        self.query_one("#metric-time", Static).update(
            _metric_card("Scan Time", time_val, "white")
        )

    def _render_lang_bar(self, d: dict) -> None:
        langs = d.get("languages", {})
        if not langs:
            self.query_one("#lang-bar-container", Static).update(
                Text(" No language data yet", style="dim")
            )
            return

        # Build a compact text bar
        COLORS = {
            "Python": "cyan", "JavaScript": "yellow", "Java": "red",
            "C": "blue", "C++": "magenta", "TypeScript": "green",
        }
        t = Text()
        t.append(" Languages: ", style="dim")
        total = sum(langs.values()) or 1
        for lang, count in sorted(langs.items(), key=lambda x: -x[1]):
            pct = count / total * 100
            color = COLORS.get(lang, "white")
            t.append(f"{lang} {pct:.0f}%  ", style=f"bold {color}")

        self.query_one("#lang-bar-container", Static).update(t)

    def _render_issues_table(self, d: dict) -> None:
        table = Table(
            "Category", "Count", "Severity",
            box=box.SIMPLE_HEAD,
            style="dim",
            header_style="bold bright_white",
            show_edge=False,
        )
        rows = [
            ("Syntax Errors",       d.get("syntax_errors", 0),    "high"),
            ("Circular Deps",       d.get("circular_deps", 0),     "high"),
            ("High Risk Files",     d.get("high_risk", 0),         "high"),
            ("Medium Risk Files",   d.get("medium_risk", 0),       "medium"),
            ("Low Risk Files",      d.get("low_risk", 0),          "low"),
        ]
        for name, count, sev in rows:
            table.add_row(name, severity_badge(count, sev), risk_text(sev) if count else Text("—", style="dim"))

        # Entry points
        eps = d.get("entry_points", [])
        if eps:
            table.add_row(
                "Entry Points",
                Text(str(len(eps)), style="bold cyan"),
                Text(", ".join(eps[:3]) + ("…" if len(eps) > 3 else ""), style="cyan"),
            )

        # Most central
        mc = d.get("most_central")
        if mc:
            table.add_row(
                "Most Central",
                Text("1", style="bold magenta"),
                Text(str(mc), style="magenta"),
            )

        self.query_one("#issues-table", Static).update(table)
