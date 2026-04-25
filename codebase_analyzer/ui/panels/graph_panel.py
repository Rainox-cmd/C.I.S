"""
ui/panels/graph_panel.py
Phase 5: Full dependency graph panel.

Displays the matplotlib PNG rendered by GraphRenderer.
Handles layout switching, file highlighting, and node detail sidebar.
Falls back gracefully to ASCII when image display is unavailable.
"""

from pathlib import Path
from textual.app import ComposeResult
from textual.widget import Widget
from textual.widgets import Static, Label, Select, Button
from textual.containers import Vertical, Horizontal, ScrollableContainer
from textual.reactive import reactive
from rich.text import Text

from utils.logger import get_logger

log = get_logger(__name__)

# ─── Constants ────────────────────────────────────────────────────────────────

RISK_STYLE = {
    "high":   ("▲", "bold red"),
    "medium": ("◆", "bold yellow"),
    "low":    ("·", "dim green"),
}

LANG_COLOR = {
    "Python":     "cyan",
    "JavaScript": "yellow",
    "TypeScript": "bright_blue",
    "Java":       "red",
    "C":          "magenta",
    "C++":        "bright_magenta",
    "Go":         "bright_cyan",
    "default":    "white",
}

_EMPTY_STATE = (
    "\n"
    "  [dim]No graph loaded.[/dim]\n\n"
    "  [dim]Analyze a project to generate\n"
    "  the dependency graph.[/dim]\n"
)

_LOADING_STATE = (
    "\n"
    "  [cyan]Rendering dependency graph…[/cyan]\n\n"
    "  [dim]Building layout and writing PNG…[/dim]\n"
)

LAYOUT_OPTIONS = [
    ("Spring (default)", "spring"),
    ("Circular",         "circular"),
    ("Hierarchy",        "hierarchy"),
    ("Spectral",         "spectral"),
    ("Kamada-Kawai",     "kamada"),
]


class GraphPanel(Widget):
    """
    Phase 5 dependency graph panel.

    Left area  → graph view (ASCII representation + PNG path notice)
    Right area → node detail sidebar (risk, connections, reasoning)
    Bottom bar → node/edge stats + layout in use

    Messages posted to main_window:
        LayoutChanged(layout)
        HighlightChanged(rel_path)
        RerenderRequested()
    """

    _layout:    reactive[str]  = reactive("spring")
    _has_graph: reactive[bool] = reactive(False)

    DEFAULT_CSS = """
    GraphPanel {
        height: 100%;
        background: $panel;
        layout: horizontal;
    }

    #graph-main {
        width: 1fr;
        height: 100%;
        layout: vertical;
    }

    #graph-title {
        background: $primary-darken-2;
        color: $text;
        padding: 0 1;
        text-style: bold;
        height: 1;
        width: 100%;
    }

    #graph-controls {
        height: 3;
        margin: 0 1;
        align: left middle;
    }

    #layout-select {
        width: 22;
        margin-right: 1;
    }

    #btn-rerender {
        width: 12;
        background: $primary-darken-2;
        border: none;
        color: $text;
        margin-right: 1;
    }

    #btn-rerender:hover { background: $primary; }

    #btn-labels {
        width: 11;
        background: $panel-darken-1;
        border: none;
        color: $text-muted;
    }

    #btn-labels:hover  { background: $boost; }
    #btn-labels.active { color: $success; }

    #graph-view {
        height: 1fr;
        margin: 0 1;
        background: $surface;
        border: tall $primary-darken-3;
        overflow: auto;
        scrollbar-size: 1 1;
        padding: 1 2;
    }

    #graph-stats {
        height: 1;
        padding: 0 1;
        background: $panel-darken-1;
        color: $text-muted;
    }

    /* ── Node detail sidebar ── */
    #node-detail {
        width: 28;
        min-width: 20;
        height: 100%;
        background: $panel-darken-1;
        border-left: tall $primary-darken-3;
        layout: vertical;
    }

    #detail-title {
        height: 1;
        background: $surface;
        color: $text-muted;
        padding: 0 1;
        text-style: bold;
    }

    #detail-scroll {
        height: 1fr;
        overflow-y: auto;
        scrollbar-size: 1 1;
        padding: 0 1;
    }

    #detail-content { padding: 0; }

    #legend-section {
        height: auto;
        padding: 0 1 1 1;
        border-top: tall $primary-darken-3;
    }
    """

    # ── Messages ──────────────────────────────────────────────────────────────

    class LayoutChanged:
        def __init__(self, layout: str):
            self.layout = layout

    class HighlightChanged:
        def __init__(self, rel_path: str | None):
            self.rel_path = rel_path

    class RerenderRequested:
        pass

    # ── Init ──────────────────────────────────────────────────────────────────

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._show_labels   = True
        self._node_meta     = {}
        self._selected_node: str | None = None
        self._render_result = None

    # ── Compose ───────────────────────────────────────────────────────────────

    def compose(self) -> ComposeResult:
        with Vertical(id="graph-main"):
            yield Static(" 🕸  DEPENDENCY GRAPH", id="graph-title")

            with Horizontal(id="graph-controls"):
                yield Select(
                    [(label, val) for label, val in LAYOUT_OPTIONS],
                    value="spring",
                    id="layout-select",
                    allow_blank=False,
                )
                yield Button("⟳ Re-render", id="btn-rerender")
                yield Button("Labels ✓",   id="btn-labels", classes="active")

            yield Static(id="graph-view")
            yield Static("No graph loaded", id="graph-stats")

        with Vertical(id="node-detail"):
            yield Static(" Node Detail", id="detail-title")
            with ScrollableContainer(id="detail-scroll"):
                yield Static(id="detail-content")
            with Vertical(id="legend-section"):
                yield Static(self._build_legend())

    def on_mount(self) -> None:
        self._show_empty()

    # ── Public API ────────────────────────────────────────────────────────────

    def load_render_result(self, render_result) -> None:
        """
        Called by main_window after GraphRenderer.render() completes.
        Accepts a RenderResult dataclass.
        """
        self._render_result = render_result
        self._node_meta     = render_result.node_meta
        self._has_graph     = True

        view = self.query_one("#graph-view", Static)

        # Build display content
        content_parts = []

        if render_result.image_path and render_result.image_path.exists():
            content_parts.append(
                f"  [bold cyan]✔ Graph rendered[/bold cyan]  "
                f"[dim]({render_result.node_count} nodes · "
                f"{render_result.edge_count} edges · "
                f"{render_result.layout_used} layout)[/dim]\n"
            )
            content_parts.append(
                f"  [dim]PNG saved to:[/dim]\n"
                f"  [cyan]{render_result.image_path}[/cyan]\n"
                f"  [dim]Open in any image viewer to see the full graph.[/dim]\n\n"
            )

        if render_result.ascii_graph:
            content_parts.append(render_result.ascii_graph)

        if render_result.error:
            content_parts.append(
                f"\n  [dim red]⚠ Render warning: {render_result.error}[/dim red]"
            )

        view.update("\n".join(content_parts) if content_parts else "[dim]No output[/dim]")
        self._update_stats(render_result)

        # Re-apply highlight if one was set before re-render
        if self._selected_node and self._selected_node in self._node_meta:
            self._render_node_detail(self._selected_node)

    def highlight_file(self, rel_path: str) -> None:
        """Called when user clicks a file in the explorer."""
        self._selected_node = rel_path

        self.query_one("#detail-title", Static).update(
            f" {Path(rel_path).name}"
        )

        if rel_path in self._node_meta:
            self._render_node_detail(rel_path)
        else:
            t = Text()
            t.append(f"\n  {Path(rel_path).name}\n", style="bold cyan")
            t.append(f"  {rel_path}\n\n", style="dim")
            t.append("  [not in dep graph]\n", style="dim")
            t.append("  (config/asset file)\n", style="dim")
            self.query_one("#detail-content", Static).update(t)

        self.post_message(self.HighlightChanged(rel_path))

    def get_image_path(self) -> Path | None:
        if self._render_result:
            return self._render_result.image_path
        return None

    def get_show_labels(self) -> bool:
        return self._show_labels

    def get_current_layout(self) -> str:
        return str(self._layout)

    def show_placeholder(self) -> None:
        self.query_one("#graph-view", Static).update(_EMPTY_STATE)
        self._has_graph = False
        self.query_one("#graph-stats", Static).update(" No graph loaded")

    def show_loading(self) -> None:
        self.query_one("#graph-view", Static).update(_LOADING_STATE)
        self.query_one("#graph-stats", Static).update(" Rendering…")

    def set_status(self, message: str) -> None:
        self.query_one("#graph-stats", Static).update(f" {message}")

    # ── Event handlers ────────────────────────────────────────────────────────

    def on_select_changed(self, event: Select.Changed) -> None:
        if event.select.id == "layout-select":
            self._layout = str(event.value)
            if self._has_graph:
                self.show_loading()
                self.post_message(self.LayoutChanged(self._layout))

    def on_button_pressed(self, event: Button.Pressed) -> None:
        event.stop()
        if event.button.id == "btn-rerender":
            if self._has_graph:
                self.show_loading()
                self.post_message(self.RerenderRequested())
        elif event.button.id == "btn-labels":
            self._show_labels = not self._show_labels
            btn = self.query_one("#btn-labels", Button)
            if self._show_labels:
                btn.label = "Labels ✓"
                btn.add_class("active")
            else:
                btn.label = "Labels ✗"
                btn.remove_class("active")
            if self._has_graph:
                self.show_loading()
                self.post_message(self.RerenderRequested())

    # ── Internal rendering ────────────────────────────────────────────────────

    def _render_node_detail(self, rel_path: str) -> None:
        meta = self._node_meta.get(rel_path)
        if not meta:
            return

        t = Text()
        t.append(f"\n  {Path(rel_path).name}\n", style="bold cyan")
        t.append(f"  {rel_path}\n\n", style="dim")

        # Language
        lang_color = LANG_COLOR.get(meta.language, "white")
        t.append("  Language   ", style="dim")
        t.append(f"{meta.language}\n", style=f"bold {lang_color}")

        # Risk
        icon, risk_style = RISK_STYLE.get(meta.risk_level, ("·", "white"))
        t.append("  Risk       ", style="dim")
        t.append(f"{icon} {meta.risk_level.upper()}\n", style=risk_style)

        t.append("  Score      ", style="dim")
        t.append(f"{meta.risk_score:.2f}\n", style="white")

        t.append("  Centrality ", style="dim")
        t.append(f"{meta.centrality:.3f}\n", style="white")

        # Connectivity
        t.append("\n  In-degree  ", style="dim")
        t.append(f"{meta.in_degree} ", style="bold white")
        t.append("files import this\n", style="dim")

        t.append("  Out-degree ", style="dim")
        t.append(f"{meta.out_degree} ", style="bold white")
        t.append("files imported\n", style="dim")

        # Flags
        flags = []
        if meta.is_entry:    flags.append(("▶ entry-point",  "bold green"))
        if meta.is_circular: flags.append(("↺ circular-dep", "bold red"))
        if meta.is_dead:     flags.append(("◌ dead-file",    "dim"))

        if flags:
            t.append("\n  Flags\n", style="dim")
            for label, style in flags:
                t.append(f"    {label}\n", style=style)

        # Reasoning
        if meta.reasoning:
            t.append("\n  Risk Reasons\n", style="dim")
            for reason in meta.reasoning[:5]:
                # Wrap long lines
                short = reason[:36] + "…" if len(reason) > 36 else reason
                t.append(f"  • {short}\n", style="dim white")

        self.query_one("#detail-content", Static).update(t)

    def _update_stats(self, render_result) -> None:
        n = render_result.node_count
        e = render_result.edge_count
        ly = render_result.layout_used
        self.query_one("#graph-stats", Static).update(
            f" {n} nodes  ·  {e} edges  ·  {ly} layout"
            + (f"  ⚠ {render_result.error[:35]}" if render_result.error else "")
        )

    def _show_empty(self) -> None:
        self.query_one("#graph-view", Static).update(_EMPTY_STATE)

    @staticmethod
    def _build_legend() -> Text:
        t = Text()
        t.append("\n Legend\n", style="dim bold")
        items = [
            ("▶", "green",         "Entry point"),
            ("↺", "red",           "Circular dep"),
            ("◌", "bright_black",  "Dead file"),
            ("▲", "red",           "High risk"),
            ("◆", "yellow",        "Medium risk"),
            ("·", "green",         "Low risk"),
        ]
        for icon, color, label in items:
            t.append(f"  {icon} ", style=f"bold {color}")
            t.append(f"{label}\n", style="dim")
        return t
