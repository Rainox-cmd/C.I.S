"""
ui/panels/error_console.py
Bottom panel: error log, execution trace output, filter controls.
Phase 1 — layout and stub data. Phase 2/3 populate with real errors.
"""

from textual.app import ComposeResult
from textual.widget import Widget
from textual.widgets import Static, Label, Select, Button, Log
from textual.containers import Vertical, Horizontal, ScrollableContainer
from rich.text import Text
from rich.table import Table
from rich import box
from datetime import datetime


# ─── Severity styles ──────────────────────────────────────────────────────────

SEVERITY = {
    "ERROR":   ("✖", "bold red"),
    "WARNING": ("⚠", "bold yellow"),
    "INFO":    ("ℹ", "cyan"),
    "TRACE":   ("→", "dim magenta"),
    "SUCCESS": ("✔", "bold green"),
}


def _fmt_entry(severity: str, file: str, message: str, line: int | None = None) -> Text:
    icon, style = SEVERITY.get(severity, ("•", "white"))
    t = Text()
    t.append(f" {icon} ", style=style)
    t.append(f"[{severity:<7}]", style=style)
    t.append(f" {file}", style="bold white")
    if line:
        t.append(f":{line}", style="dim white")
    t.append(f"  {message}", style="white")
    return t


class ErrorConsolePanel(Widget):
    """
    Bottom panel: error and trace log.

    Phase 1 responsibilities:
    - Render filter controls (severity selector, clear button)
    - Show scrollable log area with Rich-formatted entries
    - Expose log_error(), log_trace(), log_info() API

    Phase 2/3 will call these methods with real data.
    """

    DEFAULT_CSS = """
    ErrorConsolePanel {
        height: 100%;
        background: $panel;
        padding: 0;
    }

    #console-title {
        background: $error-darken-2;
        color: $text;
        padding: 0 1;
        text-style: bold;
        width: 100%;
    }

    #console-controls {
        height: 3;
        margin: 0 1;
        align: left middle;
    }

    #filter-select {
        width: 18;
        margin-right: 1;
    }

    #btn-clear {
        width: 9;
        background: $panel-darken-1;
        border: none;
        color: $text-muted;
    }

    #btn-clear:hover {
        background: $error-darken-2;
        color: $text;
    }

    #btn-copy {
        width: 11;
        background: $panel-darken-1;
        border: none;
        color: $text-muted;
        margin-left: 1;
    }

    #error-count-label {
        margin-left: 2;
        color: $text-muted;
    }

    #log-area {
        height: 1fr;
        margin: 0 1;
        background: $surface;
        border: tall $error-darken-3;
        overflow-y: auto;
        scrollbar-size: 1 1;
    }

    #console-status {
        height: 1;
        padding: 0 1;
        color: $text-muted;
        background: $panel-darken-1;
    }
    """

    FILTER_OPTIONS = [
        ("All Levels",  "ALL"),
        ("Errors Only", "ERROR"),
        ("Warnings",    "WARNING"),
        ("Trace",       "TRACE"),
        ("Info",        "INFO"),
    ]

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._entries: list[dict] = []
        self._filter = "ALL"

    def compose(self) -> ComposeResult:
        yield Static(" ⛔ ERROR CONSOLE", id="console-title")

        with Horizontal(id="console-controls"):
            yield Label("Filter:")
            yield Select(
                [(label, val) for label, val in self.FILTER_OPTIONS],
                value="ALL",
                id="filter-select",
                allow_blank=False,
            )
            yield Button("⌫ Clear", id="btn-clear")
            yield Button("⎘ Copy", id="btn-copy")
            yield Static("0 entries", id="error-count-label")

        yield ScrollableContainer(
            Static(id="log-content"),
            id="log-area",
        )
        yield Static("Ready", id="console-status")

    def on_mount(self) -> None:
        self._refresh_log()
        self.log_info("system", "Error console initialized")

    # ── Public API ────────────────────────────────────────────────────────────

    def log_error(self, file: str, message: str, line: int | None = None) -> None:
        """Log a hard error (syntax error, runtime exception, etc.)."""
        self._add_entry("ERROR", file, message, line)

    def log_warning(self, file: str, message: str, line: int | None = None) -> None:
        self._add_entry("WARNING", file, message, line)

    def log_info(self, file: str, message: str) -> None:
        self._add_entry("INFO", file, message)

    def log_trace(self, file: str, message: str) -> None:
        """Log execution trace steps (Phase 3)."""
        self._add_entry("TRACE", file, message)

    def log_success(self, file: str, message: str) -> None:
        self._add_entry("SUCCESS", file, message)

    def clear(self) -> None:
        self._entries.clear()
        self._refresh_log()
        self.query_one("#error-count-label", Static).update("0 entries")

    def set_status(self, message: str) -> None:
        self.query_one("#console-status", Static).update(f" {message}")

    def load_errors(self, error_list: list[dict]) -> None:
        """
        Bulk load errors from backend.
        error_list: list of dicts with keys:
            severity, file, message, line (optional)
        """
        for e in error_list:
            self._add_entry(
                e.get("severity", "ERROR"),
                e.get("file", "unknown"),
                e.get("message", ""),
                e.get("line"),
                _refresh=False,
            )
        self._refresh_log()

    # ── Event handlers ────────────────────────────────────────────────────────

    def on_button_pressed(self, event: Button.Pressed) -> None:
        event.stop()
        if event.button.id == "btn-clear":
            self.clear()
        elif event.button.id == "btn-copy":
            # Phase 2: copy to clipboard via pyperclip or xclip
            self.set_status("Copy to clipboard: not yet implemented")

    def on_select_changed(self, event: Select.Changed) -> None:
        if event.select.id == "filter-select":
            self._filter = str(event.value)
            self._refresh_log()

    # ── Internal ─────────────────────────────────────────────────────────────

    def _add_entry(
        self, severity: str, file: str, message: str,
        line: int | None = None, _refresh: bool = True,
    ) -> None:
        self._entries.append({
            "severity": severity,
            "file": file,
            "message": message,
            "line": line,
            "ts": datetime.now().strftime("%H:%M:%S"),
        })
        if _refresh:
            self._refresh_log()

    def _refresh_log(self) -> None:
        visible = [
            e for e in self._entries
            if self._filter == "ALL" or e["severity"] == self._filter
        ]

        lines = Text()
        for e in visible:
            ts = Text(f" [{e['ts']}] ", style="dim")
            body = _fmt_entry(e["severity"], e["file"], e["message"], e.get("line"))
            lines.append_text(ts)
            lines.append_text(body)
            lines.append("\n")

        if not lines:
            lines = Text(" No entries to display", style="dim")

        self.query_one("#log-content", Static).update(lines)

        count = len(visible)
        total = len(self._entries)
        label = f"{count} of {total} entries" if count != total else f"{total} entries"
        self.query_one("#error-count-label", Static).update(label)

        # Scroll to bottom
        try:
            container = self.query_one("#log-area", ScrollableContainer)
            container.scroll_end(animate=False)
        except Exception:
            pass
