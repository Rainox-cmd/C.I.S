"""
ui/panels/file_explorer.py
Left sidebar panel: project file tree with language badges and risk indicators.
Phase 1 — UI structure only. Data populated in Phase 2.
"""

from textual.app import ComposeResult
from textual.widget import Widget
from textual.widgets import (
    Static,
    Label,
    ListView,
    ListItem,
    Input,
    Button,
)
from textual.containers import Vertical, Horizontal
from textual import events
from rich.text import Text


# ─── Language color map (used for badges) ─────────────────────────────────────

LANG_STYLE = {
    "Python":     ("py",  "bold cyan"),
    "JavaScript": ("js",  "bold yellow"),
    "Java":       ("java","bold red"),
    "C":          ("c",   "bold blue"),
    "C++":        ("cpp", "bold magenta"),
    "TypeScript": ("ts",  "bold green"),
    "HTML":       ("html","dim white"),
    "CSS":        ("css", "dim cyan"),
    "JSON":       ("json","dim yellow"),
    "YAML":       ("yaml","dim white"),
    "Markdown":   ("md",  "dim white"),
    "Shell":      ("sh",  "dim green"),
}

RISK_STYLE = {
    "high":   ("● ", "bold red"),
    "medium": ("● ", "bold yellow"),
    "low":    ("● ", "dim green"),
    None:     ("  ", ""),
}


class FileExplorerPanel(Widget):
    """
    Left panel: project input + scanned file tree.
    
    Responsibilities (Phase 1):
    - Render path input field
    - Render Analyze / Load ZIP buttons
    - Show placeholder file list
    - Emit 'file_selected' message when a file is clicked
    
    Phase 2 will populate the list with real scan results.
    """

    DEFAULT_CSS = """
    FileExplorerPanel {
        width: 30;
        min-width: 24;
        background: $panel;
        border-right: tall $primary-darken-3;
        padding: 0;
    }

    #explorer-title {
        background: $primary-darken-2;
        color: $text;
        padding: 0 1;
        text-style: bold;
        width: 100%;
    }

    #path-input {
        margin: 1 1 0 1;
        width: 1fr;
    }

    #btn-row {
        margin: 0 1 1 1;
        height: 3;
    }

    #btn-analyze {
        width: 1fr;
        margin-right: 1;
        background: $success-darken-1;
        color: $text;
        border: none;
        text-style: bold;
    }

    #btn-analyze:hover {
        background: $success;
    }

    #btn-zip {
        width: 7;
        background: $primary-darken-2;
        color: $text;
        border: none;
    }

    #btn-zip:hover {
        background: $primary;
    }

    #file-list {
        margin: 0;
        padding: 0;
        height: 1fr;
        overflow-y: auto;
        scrollbar-size: 1 1;
    }

    #explorer-status {
        height: 1;
        padding: 0 1;
        color: $text-muted;
        background: $panel-darken-1;
    }

    .file-item {
        padding: 0 1;
        height: 1;
    }

    .file-item:hover {
        background: $boost;
    }

    .file-item.--highlight {
        background: $primary-darken-2;
    }
    """

    # ── Messages ──────────────────────────────────────────────────────────────

    class FileSelected:
        """Posted when a file row is clicked. Carries rel_path and language."""
        def __init__(self, rel_path: str, language: str):
            self.rel_path = rel_path
            self.language = language

    class AnalyzeRequested:
        """Posted when user clicks Analyze button."""
        def __init__(self, path: str):
            self.path = path

    class ZipRequested:
        """Posted when user clicks ZIP button."""

    # ── Compose ───────────────────────────────────────────────────────────────

    def compose(self) -> ComposeResult:
        yield Static(" 📁 FILE EXPLORER", id="explorer-title")
        yield Input(
            placeholder="Enter project path…",
            id="path-input",
        )
        with Horizontal(id="btn-row"):
            yield Button("▶ Analyze", id="btn-analyze", variant="success")
            yield Button("ZIP", id="btn-zip")
        yield ListView(id="file-list")
        yield Static("No project loaded", id="explorer-status")

    # ── Event handlers ────────────────────────────────────────────────────────

    def on_button_pressed(self, event: Button.Pressed) -> None:
        event.stop()
        if event.button.id == "btn-analyze":
            path_val = self.query_one("#path-input", Input).value.strip()
            if path_val:
                self.post_message(self.AnalyzeRequested(path_val))
        elif event.button.id == "btn-zip":
            self.post_message(self.ZipRequested())

    def on_list_view_selected(self, event: ListView.Selected) -> None:
        item = event.item
        rel_path = item.get_child_by_type(Static).renderable  # type: ignore
        self.post_message(self.FileSelected(str(rel_path), "Unknown"))

    # ── Public API (called by main_window in Phase 2) ─────────────────────────

    def load_files(self, file_records: list[dict]) -> None:
        """
        Populate the list with scan results.
        file_records: list of dicts with keys:
            rel_path, language, risk (optional)
        """
        lv = self.query_one("#file-list", ListView)
        lv.clear()

        for rec in file_records:
            rel_path = rec.get("rel_path", "")
            lang = rec.get("language", "Unknown")
            risk = rec.get("risk")

            label = self._build_file_label(rel_path, lang, risk)
            item = ListItem(Static(label), classes="file-item")
            lv.append(item)

        total = len(file_records)
        status = self.query_one("#explorer-status", Static)
        status.update(f" {total} files loaded")

    def set_status(self, message: str) -> None:
        self.query_one("#explorer-status", Static).update(f" {message}")

    def get_path_input(self) -> str:
        return self.query_one("#path-input", Input).value.strip()

    def set_path_input(self, path: str) -> None:
        self.query_one("#path-input", Input).value = path

    # ── Helpers ───────────────────────────────────────────────────────────────

    @staticmethod
    def _build_file_label(rel_path: str, lang: str, risk: str | None) -> Text:
        t = Text()
        risk_prefix, risk_style = RISK_STYLE.get(risk, RISK_STYLE[None])
        if risk_style:
            t.append(risk_prefix, style=risk_style)
        else:
            t.append(risk_prefix)

        _, lang_style = LANG_STYLE.get(lang, ("?", "dim white"))
        # Truncate long paths for narrow panel
        display = rel_path if len(rel_path) <= 24 else "…" + rel_path[-22:]
        t.append(display, style=lang_style)
        return t
