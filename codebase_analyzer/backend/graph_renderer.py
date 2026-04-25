"""
backend/graph_renderer.py
Phase 5: Dependency graph rendering engine.

Takes a DependencyResult + RiskResult and produces:
  - A styled PNG via matplotlib (primary output for Textual panel)
  - An ASCII text graph (fallback when matplotlib is unavailable)
  - A node metadata dict the UI uses for click interactions

Public API:
    renderer = GraphRenderer(dep_result, risk_result, scan_result)
    render   = renderer.render(layout="spring", highlight_file=None)
    # render.image_path  → Path to PNG
    # render.ascii_graph → Rich markup string
    # render.node_meta   → dict[rel_path, NodeMeta]
"""

import math
import tempfile
from pathlib import Path
from dataclasses import dataclass, field
from collections import defaultdict

from utils.logger import get_logger
from backend.dependency_analyzer import DependencyResult
from backend.risk_engine import RiskResult
from backend.scanner import ScanResult

log = get_logger(__name__)

# ─── Optional imports ─────────────────────────────────────────────────────────

try:
    import networkx as nx
    HAS_NX = True
except ImportError:
    HAS_NX = False

try:
    import matplotlib
    matplotlib.use("Agg")          # non-interactive backend — safe for threads
    import matplotlib.pyplot as plt
    import matplotlib.patches as mpatches
    import matplotlib.patheffects as pe
    HAS_MPL = True
except ImportError:
    HAS_MPL = False


# ─── Design tokens ────────────────────────────────────────────────────────────

BG_COLOR     = "#0d1117"
PANEL_COLOR  = "#161b22"
EDGE_COLOR   = "#3d4f61"
TEXT_COLOR   = "#c9d1d9"
ACCENT_COLOR = "#00d4ff"

LANG_COLORS = {
    "Python":     "#3b82f6",
    "JavaScript": "#f59e0b",
    "TypeScript": "#6366f1",
    "Java":       "#ef4444",
    "C":          "#ec4899",
    "C++":        "#a855f7",
    "Go":         "#06b6d4",
    "Ruby":       "#dc2626",
    "PHP":        "#7c3aed",
    "HTML":       "#ea580c",
    "CSS":        "#0891b2",
    "default":    "#6b7280",
}

RISK_RING_COLORS = {
    "high":   "#f85149",
    "medium": "#e3b341",
    "low":    "#39d353",
}

LAYOUT_FNS = {
    "spring":    lambda G: nx.spring_layout(G, k=2.2, seed=42, iterations=80),
    "circular":  lambda G: nx.circular_layout(G),
    "hierarchy": lambda G: _hierarchy_layout(G),
    "spectral":  lambda G: nx.spectral_layout(G) if len(G) > 2 else nx.spring_layout(G, seed=42),
    "kamada":    lambda G: nx.kamada_kawai_layout(G) if len(G) > 1 else nx.spring_layout(G, seed=42),
}


# ─── Data types ───────────────────────────────────────────────────────────────

@dataclass
class NodeMeta:
    """UI-facing metadata for a single graph node."""
    rel_path:    str
    language:    str
    risk_level:  str
    risk_score:  float
    in_degree:   int
    out_degree:  int
    is_entry:    bool
    is_dead:     bool
    is_circular: bool
    centrality:  float
    reasoning:   list[str] = field(default_factory=list)

    def summary_line(self) -> str:
        parts = [f"[{self.language}]", f"risk={self.risk_level}"]
        if self.is_entry:    parts.append("entry-point")
        if self.is_dead:     parts.append("dead-file")
        if self.is_circular: parts.append("circular-dep")
        parts.append(f"in={self.in_degree} out={self.out_degree}")
        return "  ".join(parts)


@dataclass
class RenderResult:
    """Output of a single render call."""
    image_path:  Path | None = None   # PNG written to tmp dir
    ascii_graph: str = ""             # Rich markup fallback
    node_meta:   dict[str, NodeMeta] = field(default_factory=dict)
    layout_used: str = "spring"
    node_count:  int = 0
    edge_count:  int = 0
    error:       str | None = None


# ─── Renderer ─────────────────────────────────────────────────────────────────

class GraphRenderer:
    """
    Stateless renderer — call render() as many times as needed
    (e.g. once per layout change or highlight change).
    """

    def __init__(
        self,
        dep_result:  DependencyResult,
        risk_result: RiskResult,
        scan_result: ScanResult,
    ):
        self._dep   = dep_result
        self._risk  = risk_result
        self._scan  = scan_result
        self._tmp_dir = Path(tempfile.mkdtemp(prefix="cba_graph_"))

    # ── Public ────────────────────────────────────────────────────────────────

    def render(
        self,
        layout: str = "spring",
        highlight_file: str | None = None,
        show_labels: bool = True,
        max_nodes: int = 120,
    ) -> RenderResult:
        """
        Produce a RenderResult.
        Falls back to ASCII if matplotlib is unavailable.
        """
        result = RenderResult(layout_used=layout)

        if not HAS_NX or self._dep.graph is None:
            result.ascii_graph = self._ascii_no_graph()
            result.error = "networkx not available"
            return result

        G = self._dep.graph
        if G.number_of_nodes() == 0:
            result.ascii_graph = "[dim]No files in graph[/dim]"
            return result

        # ── Build node metadata ───────────────────────────────────────────────
        result.node_meta  = self._build_node_meta()
        result.node_count = G.number_of_nodes()
        result.edge_count = G.number_of_edges()

        # ── Subgraph if too large ─────────────────────────────────────────────
        G_render = self._prepare_graph(G, max_nodes, highlight_file)

        # ── Render ────────────────────────────────────────────────────────────
        if HAS_MPL:
            try:
                img_path = self._render_matplotlib(
                    G_render, layout, highlight_file,
                    result.node_meta, show_labels,
                )
                result.image_path = img_path
                log.info("Graph rendered: %s (%d nodes)", img_path, G_render.number_of_nodes())
            except Exception as e:
                log.error("Matplotlib render failed: %s", e)
                result.error = str(e)
                result.ascii_graph = self._ascii_fallback(G_render, result.node_meta, highlight_file)
        else:
            result.ascii_graph = self._ascii_fallback(G_render, result.node_meta, highlight_file)

        return result

    def cleanup(self) -> None:
        """Remove temp PNG files."""
        import shutil
        shutil.rmtree(self._tmp_dir, ignore_errors=True)

    # ── Node metadata ─────────────────────────────────────────────────────────

    def _build_node_meta(self) -> dict[str, NodeMeta]:
        G          = self._dep.graph
        entry_set  = set(self._dep.entry_points)
        dead_set   = set(self._dep.dead_files)
        cycle_set  = set()
        for cycle in self._dep.circular_deps:
            cycle_set.update(cycle)

        meta = {}
        for rel_path in G.nodes():
            record = self._scan.get_by_rel_path(rel_path)
            risk   = self._risk.get(rel_path)
            meta[rel_path] = NodeMeta(
                rel_path    = rel_path,
                language    = record.language if record else "Unknown",
                risk_level  = risk.risk_level if risk else "low",
                risk_score  = risk.risk_score if risk else 0.0,
                in_degree   = G.in_degree(rel_path),
                out_degree  = G.out_degree(rel_path),
                is_entry    = rel_path in entry_set,
                is_dead     = rel_path in dead_set,
                is_circular = rel_path in cycle_set,
                centrality  = self._dep.centrality.get(rel_path, 0.0),
                reasoning   = risk.reasoning if risk else [],
            )
        return meta

    # ── Graph preparation ─────────────────────────────────────────────────────

    def _prepare_graph(
        self, G, max_nodes: int, highlight_file: str | None
    ):
        """If graph is large, keep the most important nodes."""
        if G.number_of_nodes() <= max_nodes:
            return G

        log.info("Graph has %d nodes — trimming to %d most important", G.number_of_nodes(), max_nodes)

        # Score = centrality + in_degree + is_entry + is_highlighted
        scores = {}
        centrality = self._dep.centrality
        entry_set  = set(self._dep.entry_points)
        for node in G.nodes():
            scores[node] = (
                centrality.get(node, 0.0) * 10
                + G.in_degree(node)
                + (5 if node in entry_set else 0)
                + (20 if node == highlight_file else 0)
            )

        top_nodes = sorted(scores, key=scores.get, reverse=True)[:max_nodes]
        return G.subgraph(top_nodes).copy()

    # ── Matplotlib render ─────────────────────────────────────────────────────

    def _render_matplotlib(
        self,
        G,
        layout: str,
        highlight_file: str | None,
        node_meta: dict[str, NodeMeta],
        show_labels: bool,
    ) -> Path:
        fig, ax = plt.subplots(figsize=(18, 13))
        fig.patch.set_facecolor(BG_COLOR)
        ax.set_facecolor(BG_COLOR)
        ax.axis("off")

        # ── Compute layout ────────────────────────────────────────────────────
        layout_fn = LAYOUT_FNS.get(layout, LAYOUT_FNS["spring"])
        try:
            pos = layout_fn(G)
        except Exception:
            pos = nx.spring_layout(G, seed=42)

        # ── Separate nodes by category ────────────────────────────────────────
        entry_nodes    = [n for n in G if node_meta.get(n, NodeMeta(n,"","low",0,0,0,False,False,False,0)).is_entry]
        circular_nodes = [n for n in G if node_meta.get(n, NodeMeta(n,"","low",0,0,0,False,False,False,0)).is_circular]
        dead_nodes     = [n for n in G if node_meta.get(n, NodeMeta(n,"","low",0,0,0,False,False,False,0)).is_dead]
        highlight_nodes= [highlight_file] if highlight_file and highlight_file in G else []
        normal_nodes   = [
            n for n in G
            if n not in entry_nodes
            and n not in circular_nodes
            and n not in dead_nodes
            and n not in highlight_nodes
        ]

        # ── Node sizing (centrality-based) ────────────────────────────────────
        max_c = max(self._dep.centrality.values(), default=1.0) or 1.0

        def node_size(n):
            c = self._dep.centrality.get(n, 0.0)
            base = 350 + (c / max_c) * 1800
            if n == highlight_file: base *= 1.6
            return base

        def node_color(n):
            meta = node_meta.get(n)
            if not meta:
                return LANG_COLORS["default"]
            return LANG_COLORS.get(meta.language, LANG_COLORS["default"])

        # ── Draw edges ────────────────────────────────────────────────────────
        # Regular edges
        regular_edges = [(u, v) for u, v in G.edges()
                         if u not in circular_nodes and v not in circular_nodes]
        if regular_edges:
            nx.draw_networkx_edges(
                G, pos,
                edgelist=regular_edges,
                edge_color=EDGE_COLOR,
                alpha=0.55,
                arrows=True,
                arrowsize=14,
                arrowstyle="-|>",
                width=1.0,
                connectionstyle="arc3,rad=0.08",
                ax=ax,
            )

        # Circular dep edges — red dashed
        circ_edges = [(u, v) for u, v in G.edges()
                      if u in circular_nodes or v in circular_nodes]
        if circ_edges:
            nx.draw_networkx_edges(
                G, pos,
                edgelist=circ_edges,
                edge_color="#f85149",
                alpha=0.75,
                arrows=True,
                arrowsize=16,
                width=1.6,
                style="dashed",
                connectionstyle="arc3,rad=0.12",
                ax=ax,
            )

        # Highlighted file edges — accent color
        if highlight_file and highlight_file in G:
            hl_edges = [(u, v) for u, v in G.edges()
                        if u == highlight_file or v == highlight_file]
            if hl_edges:
                nx.draw_networkx_edges(
                    G, pos,
                    edgelist=hl_edges,
                    edge_color=ACCENT_COLOR,
                    alpha=0.9,
                    arrows=True,
                    arrowsize=18,
                    width=2.2,
                    connectionstyle="arc3,rad=0.08",
                    ax=ax,
                )

        # ── Draw node groups ──────────────────────────────────────────────────
        def draw_group(nodes, edgecolors, linewidths, alpha=0.92):
            if not nodes:
                return
            colors = [node_color(n) for n in nodes]
            sizes  = [node_size(n)  for n in nodes]
            nx.draw_networkx_nodes(
                G, pos,
                nodelist=nodes,
                node_color=colors,
                node_size=sizes,
                edgecolors=edgecolors,
                linewidths=linewidths,
                alpha=alpha,
                ax=ax,
            )

        draw_group(normal_nodes,    edgecolors="none",    linewidths=0)
        draw_group(dead_nodes,      edgecolors="#6b7280", linewidths=1.5)
        draw_group(entry_nodes,     edgecolors="#39d353", linewidths=2.5)
        draw_group(circular_nodes,  edgecolors="#f85149", linewidths=2.5)
        draw_group(highlight_nodes, edgecolors=ACCENT_COLOR, linewidths=3.5)

        # ── Risk rings (outer halo for high/medium) ───────────────────────────
        for n in G.nodes():
            meta = node_meta.get(n)
            if not meta or meta.risk_level == "low":
                continue
            ring_color = RISK_RING_COLORS[meta.risk_level]
            ring_size  = node_size(n) * 2.1
            nx.draw_networkx_nodes(
                G, pos,
                nodelist=[n],
                node_color="none",
                node_size=ring_size,
                edgecolors=ring_color,
                linewidths=1.2,
                alpha=0.35,
                ax=ax,
            )

        # ── Labels ────────────────────────────────────────────────────────────
        if show_labels and G.number_of_nodes() <= 60:
            labels = {n: Path(n).name for n in G.nodes()}
            # Highlight label bolder
            hl_labels   = {n: labels[n] for n in highlight_nodes if n in labels}
            norm_labels  = {n: labels[n] for n in G.nodes() if n not in highlight_nodes}

            nx.draw_networkx_labels(
                G, pos,
                labels=norm_labels,
                font_size=7.5,
                font_color=TEXT_COLOR,
                font_family="monospace",
                ax=ax,
            )
            if hl_labels:
                nx.draw_networkx_labels(
                    G, pos,
                    labels=hl_labels,
                    font_size=9,
                    font_color=ACCENT_COLOR,
                    font_family="monospace",
                    font_weight="bold",
                    ax=ax,
                )
        elif G.number_of_nodes() > 60:
            # Only label entry points and highlighted node when graph is large
            sparse_labels = {n: Path(n).name for n in entry_nodes + highlight_nodes if n in G}
            if sparse_labels:
                nx.draw_networkx_labels(
                    G, pos, labels=sparse_labels,
                    font_size=8, font_color=TEXT_COLOR,
                    font_family="monospace", ax=ax,
                )

        # ── Legend ────────────────────────────────────────────────────────────
        legend_items = [
            mpatches.Patch(color=LANG_COLORS[l], label=l)
            for l in ["Python", "JavaScript", "TypeScript", "Java", "C++"]
            if any(
                node_meta.get(n, NodeMeta("","","",0,0,0,False,False,False,0)).language == l
                for n in G.nodes()
            )
        ]
        legend_items += [
            mpatches.Patch(color="none", label="", linewidth=0),
            mpatches.Patch(edgecolor="#39d353", facecolor="none", linewidth=2, label="Entry point"),
            mpatches.Patch(edgecolor="#f85149", facecolor="none", linewidth=2, label="Circular dep"),
            mpatches.Patch(edgecolor="#6b7280", facecolor="none", linewidth=1.5, label="Dead file"),
            mpatches.Patch(edgecolor=ACCENT_COLOR, facecolor="none", linewidth=3, label="Selected"),
            mpatches.Patch(color="none", label="", linewidth=0),
            mpatches.Patch(color=RISK_RING_COLORS["high"],   label="High risk",   alpha=0.7),
            mpatches.Patch(color=RISK_RING_COLORS["medium"], label="Medium risk", alpha=0.7),
        ]
        ax.legend(
            handles=legend_items,
            loc="upper left",
            facecolor=PANEL_COLOR,
            edgecolor="#2d3741",
            labelcolor=TEXT_COLOR,
            fontsize=8.5,
            framealpha=0.90,
        )

        # ── Title ─────────────────────────────────────────────────────────────
        title = f"Dependency Graph  ·  {G.number_of_nodes()} nodes  ·  {G.number_of_edges()} edges"
        if highlight_file:
            title += f"  ·  highlighted: {Path(highlight_file).name}"
        ax.set_title(title, color=TEXT_COLOR, fontsize=11, pad=16, fontfamily="monospace")

        plt.tight_layout(pad=1.5)

        # ── Save ──────────────────────────────────────────────────────────────
        out_path = self._tmp_dir / f"graph_{layout}.png"
        plt.savefig(
            out_path,
            dpi=130,
            bbox_inches="tight",
            facecolor=BG_COLOR,
            edgecolor="none",
        )
        plt.close(fig)
        return out_path

    # ── ASCII fallback ────────────────────────────────────────────────────────

    def _ascii_fallback(
        self,
        G,
        node_meta: dict[str, NodeMeta],
        highlight_file: str | None,
    ) -> str:
        """
        Rich-markup ASCII representation of the graph.
        Used when matplotlib is unavailable or as a compact summary.
        """
        lines = []
        lines.append("[bold]Dependency Graph[/bold]  [dim](ASCII fallback)[/dim]\n")

        RISK_ICON = {"high": "[bold red]▲[/bold red]", "medium": "[bold yellow]◆[/bold yellow]", "low": "[dim green]·[/dim green]"}
        ENTRY_ICON = "[bold green]▶[/bold green]"
        CIRC_ICON  = "[bold red]↺[/bold red]"
        DEAD_ICON  = "[dim]◌[/dim]"
        HL_ICON    = f"[bold cyan]★[/bold cyan]"

        entry_set    = set(self._dep.entry_points)
        dead_set     = set(self._dep.dead_files)
        cycle_set    = set()
        for cycle in self._dep.circular_deps:
            cycle_set.update(cycle)

        for node in sorted(G.nodes()):
            meta = node_meta.get(node)
            risk = meta.risk_level if meta else "low"
            icon = RISK_ICON.get(risk, "·")

            if node == highlight_file:
                icon = HL_ICON
            elif node in entry_set:
                icon = ENTRY_ICON
            elif node in cycle_set:
                icon = CIRC_ICON
            elif node in dead_set:
                icon = DEAD_ICON

            lang  = meta.language if meta else ""
            deps  = list(G.successors(node))
            short = Path(node).name

            line = f"  {icon} [cyan]{short}[/cyan]"
            if lang:
                line += f" [dim]({lang})[/dim]"
            if deps:
                dep_names = ", ".join(Path(d).name for d in deps[:4])
                if len(deps) > 4:
                    dep_names += f" +{len(deps)-4}"
                line += f"  [dim]→ {dep_names}[/dim]"
            lines.append(line)

        lines.append("")
        lines.append("[dim]▶ entry  ↺ circular  ◌ dead  ▲ high-risk  ★ selected[/dim]")
        return "\n".join(lines)

    def _ascii_no_graph(self) -> str:
        return "[dim]networkx is required for graph visualization.\n  pip install networkx matplotlib[/dim]"


# ─── Hierarchy layout helper ──────────────────────────────────────────────────

def _hierarchy_layout(G) -> dict:
    """
    Simple top-down hierarchy: roots at top, leaves at bottom.
    Falls back to spring if graph has cycles.
    """
    try:
        # Try topological sort — fails on cyclic graphs
        topo = list(nx.topological_sort(G))
        levels: dict[str, int] = {}
        for node in topo:
            preds = list(G.predecessors(node))
            if not preds:
                levels[node] = 0
            else:
                levels[node] = max(levels.get(p, 0) for p in preds) + 1

        max_level = max(levels.values(), default=0) or 1
        level_counts: dict[int, int] = defaultdict(int)
        level_pos:    dict[int, int] = defaultdict(int)

        for node in topo:
            level_counts[levels[node]] += 1

        pos = {}
        for node in topo:
            lvl = levels[node]
            count = level_counts[lvl]
            idx   = level_pos[lvl]
            x = (idx - count / 2) * (2.0 / max(count, 1))
            y = 1.0 - lvl / max_level
            pos[node] = (x, y)
            level_pos[lvl] += 1

        return pos
    except nx.NetworkXUnfeasible:
        # Has cycles — fall back to spring
        return nx.spring_layout(G, k=2.2, seed=42)
