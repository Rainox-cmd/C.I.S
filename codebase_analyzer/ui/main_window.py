"""
ui/main_window.py
Main Textual application.
Assembles all panels, routes messages between them, manages app state.

Phase 1: Pure UI shell — no backend calls.
Phase 2: Will wire backend scanner into _run_analysis().
"""

import asyncio
import time
from pathlib import Path
from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, Static, TabbedContent, TabPane
from textual.containers import Horizontal, Vertical
from textual.binding import Binding
from textual import work
from rich.text import Text

from ui.panels.file_explorer import FileExplorerPanel
from ui.panels.dashboard import DashboardPanel
from ui.panels.graph_panel import GraphPanel
from ui.panels.error_console import ErrorConsolePanel
from ui.panels.chat_panel import ChatPanel
from utils.logger import get_logger
from utils.file_handler import resolve_project_path, FileHandlerError

log = get_logger(__name__)

# Backend imports — guarded so UI shell still runs if deps missing
try:
    from backend.scanner import ProjectScanner
    from backend.dependency_analyzer import DependencyAnalyzer
    from backend.error_detector import ErrorDetector
    from backend.execution_tracer import ExecutionTracer
    from backend.risk_engine import RiskEngine
    from backend.graph_renderer import GraphRenderer
    BACKEND_AVAILABLE = True
except ImportError as _be:
    BACKEND_AVAILABLE = False
    log.warning("Backend not fully importable: %s", _be)

# AI module imports — guarded; AI is optional enhancement
try:
    from ai.nim_client import NIMClient, NIMError, NIMAuthError, NIMConnectionError
    from ai.context_builder import ContextBuilder
    from ai.prompt_engine import PromptEngine
    AI_AVAILABLE = True
except ImportError as _ai:
    AI_AVAILABLE = False
    log.warning("AI modules not fully importable: %s", _ai)

# ─── App-level CSS ────────────────────────────────────────────────────────────

APP_CSS = """
Screen {
    background: $background;
    layout: horizontal;
}

/* ── Left sidebar ── */
#left-sidebar {
    width: 30;
    height: 100%;
    dock: left;
}

/* ── Right chat sidebar ── */
#right-sidebar {
    width: 36;
    height: 100%;
    dock: right;
}

/* ── Main center area ── */
#main-area {
    height: 100%;
    layout: vertical;
}

/* ── Top: dashboard ── */
#top-pane {
    height: 55%;
    min-height: 18;
}

/* ── Bottom: tabs for graph + console ── */
#bottom-tabs {
    height: 45%;
    min-height: 12;
}

/* ── Tabbed content ── */
TabbedContent {
    height: 100%;
}

ContentSwitcher {
    height: 1fr;
}

/* ── Loading overlay ── */
#loading-overlay {
    display: none;
    layer: overlay;
    align: center middle;
    background: rgba(0,0,0,0.7);
}

#loading-overlay.--visible {
    display: block;
}

/* ── Status bar ── */
#global-status {
    dock: bottom;
    height: 1;
    background: $primary-darken-3;
    color: $text-muted;
    padding: 0 1;
}
"""


class CodebaseAnalyzerApp(App):
    """
    Main application class.
    Textual entry point — run with: app = CodebaseAnalyzerApp(); app.run()
    """

    TITLE = "Codebase Analyzer  v1.0"
    SUB_TITLE = "AI-Assisted Local Code Intelligence"
    CSS = APP_CSS

    BINDINGS = [
        Binding("ctrl+q", "quit",          "Quit",          show=True),
        Binding("ctrl+a", "focus_analyze", "Analyze",       show=True),
        Binding("ctrl+c", "focus_chat",    "Chat",          show=True),
        Binding("ctrl+g", "focus_graph",   "Graph",         show=True),
        Binding("ctrl+e", "focus_errors",  "Errors",        show=True),
        Binding("ctrl+d", "toggle_chat",   "Toggle Chat",   show=True),
        Binding("ctrl+p", "show_demo",     "Demo Data",     show=False),
        Binding("f1",     "show_help",     "Help",          show=True),
    ]

    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._project_path: Path | None = None
        self._analysis_running = False
        self._chat_visible = True
        # Populated after analysis — used by Phase 6 AI context builder
        self._scan_result  = None
        self._dep_result   = None
        self._error_result = None
        self._trace_result = None
        self._risk_result  = None
        self._graph_renderer: "GraphRenderer | None" = None
        # AI components (Phase 6)
        self._nim_client:      "NIMClient | None"      = None
        self._context_builder: "ContextBuilder | None" = None
        self._prompt_engine:   "PromptEngine | None"   = None

    # ── Compose ───────────────────────────────────────────────────────────────

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)

        # Left: file explorer
        with Vertical(id="left-sidebar"):
            yield FileExplorerPanel(id="file-explorer")

        # Center: dashboard top, tabbed panels bottom
        with Vertical(id="main-area"):
            with Vertical(id="top-pane"):
                yield DashboardPanel(id="dashboard")

            with TabbedContent(id="bottom-tabs"):
                with TabPane("🕸  Graph", id="tab-graph"):
                    yield GraphPanel(id="graph-panel")
                with TabPane("⛔ Errors", id="tab-errors"):
                    yield ErrorConsolePanel(id="error-console")

        # Right: chat panel (dockable)
        with Vertical(id="right-sidebar"):
            yield ChatPanel(id="chat-panel")

        yield Footer()

    # ── Lifecycle ─────────────────────────────────────────────────────────────

    def on_mount(self) -> None:
        log.info("Application mounted")
        self._set_global_status(
            "Ready  |  Ctrl+A: Analyze  |  Ctrl+C: Chat  |  F1: Help"
        )
        self.query_one("#graph-panel", GraphPanel).show_placeholder()
        console = self.query_one("#error-console", ErrorConsolePanel)
        console.log_info("system", "Codebase Analyzer started")
        console.log_info("system", "Enter a project path and click ▶ Analyze")
        # Try to connect AI on startup if API key is available
        self._try_connect_ai()

    # ── Message handlers from panels ──────────────────────────────────────────

    def on_file_explorer_panel_analyze_requested(
        self, event: FileExplorerPanel.AnalyzeRequested
    ) -> None:
        """User clicked Analyze button in explorer."""
        log.info("Analyze requested: %s", event.path)
        if not self._analysis_running:
            self._start_analysis(event.path)

    def on_file_explorer_panel_zip_requested(
        self, event: FileExplorerPanel.ZipRequested
    ) -> None:
        """User clicked ZIP button — prompt for ZIP path (Phase 2)."""
        console = self.query_one("#error-console", ErrorConsolePanel)
        console.log_info("ui", "ZIP input: not yet implemented (Phase 2)")
        self._set_global_status("ZIP input coming in Phase 2")

    def on_file_explorer_panel_file_selected(
        self, event: FileExplorerPanel.FileSelected
    ) -> None:
        """User clicked a file in the explorer."""
        log.debug("File selected: %s", event.rel_path)
        # Highlight in graph
        self.query_one("#graph-panel", GraphPanel).highlight_file(event.rel_path)
        # Inject into chat context
        self.query_one("#chat-panel", ChatPanel).inject_file_context(event.rel_path)
        self._set_global_status(f"Selected: {event.rel_path}")

    def on_chat_panel_message_sent(self, event: ChatPanel.MessageSent) -> None:
        """User sent a chat message — run the full AI pipeline."""
        log.info("Chat message: %r | context: %s", event.text[:60], event.context_file)
        if self._nim_client and self._context_builder and self._prompt_engine:
            self._run_ai_query(event.text, event.context_file, event.history)
        else:
            chat = self.query_one("#chat-panel", ChatPanel)
            chat.append_error_message(
                "AI not connected. Set NVIDIA_API_KEY environment variable and restart."
            )

    # ── AI connection ─────────────────────────────────────────────────────────

    def _try_connect_ai(self) -> None:
        """
        Read API key from environment and attempt connection.
        Called on startup — silently skips if no key is set.
        """
        import os
        api_key = os.environ.get("NVIDIA_API_KEY", "").strip()
        if not api_key:
            log.info("NVIDIA_API_KEY not set — AI chat disabled")
            self.query_one("#error-console", ErrorConsolePanel).log_info(
                "ai", "AI chat disabled — set NVIDIA_API_KEY to enable"
            )
            return
        self._connect_ai(api_key)

    @work(exclusive=False)
    async def _connect_ai(self, api_key: str) -> None:
        """Connect to NVIDIA NIM and run health check."""
        console  = self.query_one("#error-console", ErrorConsolePanel)
        chat     = self.query_one("#chat-panel", ChatPanel)

        if not AI_AVAILABLE:
            console.log_error("ai", "AI modules unavailable — pip install openai")
            return

        console.log_info("ai", "Connecting to NVIDIA NIM…")
        try:
            client = NIMClient(api_key)
            ok, model_name = await client.health_check()

            if ok:
                self._nim_client    = client
                self._prompt_engine = PromptEngine()
                # ContextBuilder needs analysis results — built after analysis
                chat.set_ai_connected(model_name, client.is_using_fallback)
                console.log_success("ai", f"Connected: {model_name}")
                self._set_global_status(f"AI: {model_name.split('/')[-1]}")
            else:
                console.log_error("ai", f"AI health check failed: {model_name}")
                chat.set_ai_disconnected(model_name)

        except NIMAuthError as e:
            console.log_error("ai", f"Auth error: {e}")
            chat.set_ai_disconnected("auth error")
        except Exception as e:
            console.log_error("ai", f"Connection error: {e}")
            chat.set_ai_disconnected(str(e)[:60])

    def _rebuild_context_builder(self) -> None:
        """Called after analysis completes to rebuild the ContextBuilder."""
        if (self._scan_result and self._dep_result and
                self._error_result and self._trace_result and self._risk_result):
            self._context_builder = ContextBuilder(
                self._scan_result,
                self._dep_result,
                self._error_result,
                self._trace_result,
                self._risk_result,
            )
            log.info("ContextBuilder rebuilt")

    # ── AI query worker ───────────────────────────────────────────────────────

    @work(exclusive=False)
    async def _run_ai_query(
        self,
        question:     str,
        context_file: str | None,
        history:      list[dict],
    ) -> None:
        """
        Full AI pipeline:
          1. Build context from analysis results
          2. Format prompt
          3. Stream response tokens to chat panel
          4. Parse and display confidence score
        """
        chat    = self.query_one("#chat-panel", ChatPanel)
        console = self.query_one("#error-console", ErrorConsolePanel)

        if not self._context_builder:
            # Analysis may not have run yet
            if (self._scan_result and self._dep_result):
                self._rebuild_context_builder()
            else:
                chat.append_error_message(
                    "No project analyzed yet. Run an analysis first."
                )
                return

        try:
            # ── 1. Build context ──────────────────────────────────────────────
            context = await asyncio.get_event_loop().run_in_executor(
                None,
                lambda: self._context_builder.build(question, context_file),
            )
            log.info(
                "Context built: %d files, ~%d tokens",
                len(context.files), context.estimated_tokens,
            )

            # ── 2. Format prompt ──────────────────────────────────────────────
            messages = self._prompt_engine.build_followup_messages(context, history)

            # ── 3. Stream response ────────────────────────────────────────────
            chat.stream_start()
            full_response = []

            async for chunk in self._nim_client.stream(
                messages=[m for m in messages if m["role"] != "system"],
                system_prompt=next(
                    (m["content"] for m in messages if m["role"] == "system"), None
                ),
            ):
                full_response.append(chunk)
                chat.stream_chunk(chunk)
                await asyncio.sleep(0)   # yield to UI between chunks

            # ── 4. Parse confidence and finalise ─────────────────────────────
            full_text  = "".join(full_response)
            confidence = self._prompt_engine.parse_confidence(full_text)
            chat.stream_end(confidence)

            console.log_info(
                "ai",
                f"Response: ~{len(full_text.split())} words"
                + (f", confidence={confidence}%" if confidence is not None else ""),
            )

        except NIMAuthError as e:
            chat.append_error_message(f"Auth error: {e}")
            chat.set_ai_disconnected("auth error")
        except NIMConnectionError as e:
            chat.append_error_message(f"Connection error: {e}")
        except Exception as e:
            log.exception("AI query failed: %s", e)
            chat.append_error_message(f"Error: {e}")

    # ── Graph panel message handlers ──────────────────────────────────────────

    def on_graph_panel_layout_changed(
        self, event: "GraphPanel.LayoutChanged"
    ) -> None:
        """User changed the graph layout dropdown."""
        if self._graph_renderer:
            self._render_graph(layout=event.layout)

    def on_graph_panel_rerender_requested(
        self, event: "GraphPanel.RerenderRequested"
    ) -> None:
        """User clicked Re-render button."""
        if self._graph_renderer:
            graph_panel = self.query_one("#graph-panel", GraphPanel)
            self._render_graph(
                layout=graph_panel.get_current_layout(),
                highlight_file=graph_panel._selected_node,
                show_labels=graph_panel.get_show_labels(),
            )

    def on_graph_panel_highlight_changed(
        self, event: "GraphPanel.HighlightChanged"
    ) -> None:
        """File highlighted in graph — re-render with highlight."""
        if self._graph_renderer and event.rel_path:
            graph_panel = self.query_one("#graph-panel", GraphPanel)
            self._render_graph(
                layout=graph_panel.get_current_layout(),
                highlight_file=event.rel_path,
                show_labels=graph_panel.get_show_labels(),
            )

    @work(exclusive=False)
    async def _render_graph(
        self,
        layout: str = "spring",
        highlight_file: str | None = None,
        show_labels: bool = True,
    ) -> None:
        """Render the dependency graph in a background worker."""
        if not self._graph_renderer:
            return

        renderer = self._graph_renderer
        graph_panel = self.query_one("#graph-panel", GraphPanel)

        try:
            render_result = await asyncio.get_event_loop().run_in_executor(
                None,
                lambda: renderer.render(
                    layout=layout,
                    highlight_file=highlight_file,
                    show_labels=show_labels,
                ),
            )
            graph_panel.load_render_result(render_result)

            console = self.query_one("#error-console", ErrorConsolePanel)
            if render_result.image_path:
                console.log_success(
                    "graph",
                    f"Graph rendered: {render_result.node_count} nodes, "
                    f"{render_result.edge_count} edges → {render_result.image_path.name}",
                )
            if render_result.error:
                console.log_warning("graph", f"Render warning: {render_result.error}")

        except Exception as e:
            log.error("Graph render error: %s", e)
            graph_panel.set_status(f"⚠ Render failed: {e}")

    # ── Analysis orchestration ────────────────────────────────────────────────

    def _start_analysis(self, path: str) -> None:
        """
        Entry point for analysis workflow.
        Phase 1: validates path and shows placeholder.
        Phase 2+: calls real backend modules.
        """
        from pathlib import Path as P
        p = P(path.strip()).expanduser().resolve()

        console = self.query_one("#error-console", ErrorConsolePanel)
        dashboard = self.query_one("#dashboard", DashboardPanel)
        explorer = self.query_one("#file-explorer", FileExplorerPanel)

        if not p.exists():
            console.log_error("input", f"Path not found: {p}")
            self._set_global_status(f"⚠ Path not found: {p}")
            return

        self._project_path = p
        self._analysis_running = True
        self._set_global_status(f"Analyzing: {p}  …")
        dashboard.set_status("Scanning…")
        explorer.set_status("Scanning…")
        console.log_info("scanner", f"Starting scan: {p}")

        # Phase 1: simulate async scan with placeholder data
        self._run_analysis_stub(p)

    @work(exclusive=True)
    async def _run_analysis_stub(self, path: Path) -> None:
        """
        Phase 2: Real backend pipeline.
        Runs in a Textual worker (separate thread) so UI never freezes.
        Sequence: Scanner → DependencyAnalyzer → ErrorDetector → ExecutionTracer → RiskEngine
        """
        console   = self.query_one("#error-console", ErrorConsolePanel)
        dashboard = self.query_one("#dashboard", DashboardPanel)
        explorer  = self.query_one("#file-explorer", FileExplorerPanel)

        t_start = time.perf_counter()

        if not BACKEND_AVAILABLE:
            console.log_error("backend", "Backend modules not importable — check dependencies")
            self._analysis_running = False
            return

        try:
            # ── Resolve path (handles ZIP too) ────────────────────────────────
            try:
                resolved_path = resolve_project_path(str(path))
            except FileHandlerError as e:
                console.log_error("input", str(e))
                self._analysis_running = False
                return

            # ── Stage 1: Scan ─────────────────────────────────────────────────
            console.log_info("scanner", f"Scanning: {resolved_path}")
            dashboard.set_status("Stage 1/5 — Scanning files…")
            await asyncio.sleep(0)   # yield to UI

            scanner     = ProjectScanner(resolved_path)
            scan_result = await asyncio.get_event_loop().run_in_executor(
                None, scanner.scan
            )
            console.log_success("scanner", f"Found {len(scan_result.files)} files")

            # ── Stage 2: Dependency analysis ──────────────────────────────────
            console.log_info("deps", "Building dependency graph…")
            dashboard.set_status("Stage 2/5 — Analyzing dependencies…")
            await asyncio.sleep(0)

            dep_analyzer = DependencyAnalyzer(scan_result)
            dep_result   = await asyncio.get_event_loop().run_in_executor(
                None, dep_analyzer.analyze
            )
            console.log_success(
                "deps",
                f"Graph: {len(dep_result.edges)} edges, "
                f"{len(dep_result.circular_deps)} circular dep(s)",
            )

            # ── Stage 3: Error detection ──────────────────────────────────────
            console.log_info("errors", "Running static error detection…")
            dashboard.set_status("Stage 3/5 — Detecting issues…")
            await asyncio.sleep(0)

            error_detector = ErrorDetector(scan_result, dep_result)
            error_result   = await asyncio.get_event_loop().run_in_executor(
                None, error_detector.detect
            )
            console.log_success("errors", f"Found {len(error_result.issues)} issue(s)")

            # ── Stage 4: Execution tracing ────────────────────────────────────
            console.log_info("tracer", "Running safe execution checks (Python only)…")
            dashboard.set_status("Stage 4/5 — Tracing execution…")
            await asyncio.sleep(0)

            tracer       = ExecutionTracer(scan_result, dep_result)
            trace_result = await asyncio.get_event_loop().run_in_executor(
                None, tracer.trace
            )
            console.log_success(
                "tracer",
                f"Traced {len(trace_result.file_results)} Python files, "
                f"{len(trace_result.failed_files())} failed",
            )

            # ── Stage 5: Risk scoring ─────────────────────────────────────────
            console.log_info("risk", "Calculating risk scores…")
            dashboard.set_status("Stage 5/5 — Scoring risk…")
            await asyncio.sleep(0)

            risk_engine = RiskEngine(scan_result, dep_result, error_result, trace_result)
            risk_result = await asyncio.get_event_loop().run_in_executor(
                None, risk_engine.score
            )
            counts = risk_result.counts()
            console.log_success(
                "risk",
                f"Risk: {counts['high']} high, {counts['medium']} medium, {counts['low']} low",
            )

            # ── Load execution errors into console ────────────────────────────
            console.load_errors(error_result.for_console())
            if trace_result.file_results:
                console.load_errors(trace_result.for_console())
            for chain in trace_result.failure_chains:
                console.log_trace(
                    chain["root"],
                    f"Failure chain: impacts {len(chain['impacts'])} file(s) — "
                    + ", ".join(chain["impacts"][:3])
                    + ("…" if len(chain["impacts"]) > 3 else ""),
                )

            # ── Build UI payloads ─────────────────────────────────────────────
            elapsed = round(time.perf_counter() - t_start, 2)

            issue_counts = error_result.count_by_severity()
            summary_data = {
                "total_files":    len(scan_result.files),
                "total_issues":   len(error_result.issues),
                "languages":      scan_result.language_counts,
                "high_risk":      counts["high"],
                "medium_risk":    counts["medium"],
                "low_risk":       counts["low"],
                "circular_deps":  len(dep_result.circular_deps),
                "syntax_errors":  len(error_result.by_type("syntax_error")),
                "entry_points":   dep_result.entry_points,
                "most_central":   dep_result.most_central,
                "analysis_time":  elapsed,
            }

            file_records_for_ui = [
                {
                    "rel_path": rec.rel_path,
                    "language": rec.language,
                    "risk":     rec.risk or "low",
                }
                for rec in scan_result.files
            ]

            # ── Update UI ─────────────────────────────────────────────────────
            explorer.load_files(file_records_for_ui)
            explorer.set_status(f"{len(scan_result.files)} files | {resolved_path.name}")
            dashboard.update_summary(summary_data)

            # Store results on app for chat context (Phase 6)
            self._scan_result  = scan_result
            self._dep_result   = dep_result
            self._error_result = error_result
            self._trace_result = trace_result
            self._risk_result  = risk_result

            # ── Stage 6: Graph rendering ──────────────────────────────────────
            console.log_info("graph", "Rendering dependency graph…")
            dashboard.set_status("Rendering graph…")

            if BACKEND_AVAILABLE:
                # Clean up previous renderer temp files
                if self._graph_renderer:
                    self._graph_renderer.cleanup()

                self._graph_renderer = GraphRenderer(dep_result, risk_result, scan_result)
                graph_panel = self.query_one("#graph-panel", GraphPanel)
                graph_panel.show_loading()

                render_result = await asyncio.get_event_loop().run_in_executor(
                    None,
                    lambda: self._graph_renderer.render(layout="spring", show_labels=True),
                )
                graph_panel.load_render_result(render_result)

                if render_result.image_path:
                    console.log_success(
                        "graph",
                        f"Graph PNG: {render_result.image_path.name}  "
                        f"({render_result.node_count} nodes, {render_result.edge_count} edges)",
                    )

            self._analysis_running = False

            # ── Rebuild AI context builder with fresh analysis results ─────────
            self._rebuild_context_builder()
            if self._nim_client and self._context_builder:
                console.log_info("ai", "AI context ready for this project")

            self._set_global_status(
                f"✔ Complete: {resolved_path.name}  |  "
                f"{len(scan_result.files)} files  |  "
                f"{len(error_result.issues)} issues  |  "
                f"{elapsed}s"
            )
            log.info("Phase 2 analysis pipeline complete in %.2fs", elapsed)

        except Exception as e:
            log.exception("Analysis pipeline error: %s", e)
            console.log_error("pipeline", f"Unexpected error: {e}")
            self._analysis_running = False
            self._set_global_status(f"⚠ Analysis failed: {e}")

    # ── Key bindings ──────────────────────────────────────────────────────────

    def action_focus_analyze(self) -> None:
        self.query_one("#path-input").focus()

    def action_focus_chat(self) -> None:
        self.query_one("#chat-input").focus()

    def action_focus_graph(self) -> None:
        self.query_one("#bottom-tabs").active = "tab-graph"

    def action_focus_errors(self) -> None:
        self.query_one("#bottom-tabs").active = "tab-errors"

    def action_toggle_chat(self) -> None:
        sidebar = self.query_one("#right-sidebar")
        self._chat_visible = not self._chat_visible
        sidebar.display = self._chat_visible
        self._set_global_status(
            "Chat panel shown" if self._chat_visible else "Chat panel hidden (Ctrl+D to show)"
        )

    def action_show_demo(self) -> None:
        """Ctrl+P: load demo data without scanning a real path."""
        self._start_analysis("/demo/project")

    def action_show_help(self) -> None:
        console = self.query_one("#error-console", ErrorConsolePanel)
        help_lines = [
            ("INFO", "help", "Ctrl+A → Focus path input / Analyze"),
            ("INFO", "help", "Ctrl+C → Focus chat input"),
            ("INFO", "help", "Ctrl+G → Switch to Graph tab"),
            ("INFO", "help", "Ctrl+E → Switch to Error Console tab"),
            ("INFO", "help", "Ctrl+D → Toggle chat sidebar"),
            ("INFO", "help", "Ctrl+P → Load demo data"),
            ("INFO", "help", "F1    → Show this help"),
            ("INFO", "help", "Ctrl+Q → Quit"),
        ]
        for sev, file, msg in help_lines:
            console.log_info(file, msg)
        self.query_one("#bottom-tabs").active = "tab-errors"

    # ── Helpers ───────────────────────────────────────────────────────────────

    def _set_global_status(self, message: str) -> None:
        try:
            self.sub_title = message
        except Exception:
            pass
