#!/usr/bin/env python3
"""
main.py
Entry point for the Codebase Analyzer.

Usage:
    python main.py                        # Launch Textual UI
    python main.py --path /my/project     # CLI analyze mode
    python main.py --demo                 # UI with demo data pre-loaded
    python main.py --help                 # Show CLI options
"""

import sys
import argparse
from pathlib import Path

from utils.logger import get_logger, get_log_path

log = get_logger(__name__)


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="codebase-analyzer",
        description="AI-Assisted Local Codebase Analyzer",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python main.py                          Launch the full Textual UI
  python main.py --path ./my_project      Analyze a folder in CLI mode
  python main.py --path project.zip       Analyze a ZIP file in CLI mode
  python main.py --demo                   Open UI with demo data
        """,
    )
    p.add_argument(
        "--path", "-p",
        metavar="PATH",
        help="Project folder or ZIP to analyze (CLI mode, no UI)",
    )
    p.add_argument(
        "--demo",
        action="store_true",
        help="Open UI and immediately load demo data",
    )
    p.add_argument(
        "--no-chat",
        action="store_true",
        help="Start UI with chat panel hidden",
    )
    p.add_argument(
        "--log-level",
        choices=["DEBUG", "INFO", "WARNING", "ERROR"],
        default="INFO",
        help="Log verbosity (default: INFO)",
    )
    p.add_argument(
        "--version",
        action="version",
        version="Codebase Analyzer 1.0.0 — Phase 1",
    )
    return p


def run_ui(demo: bool = False, no_chat: bool = False) -> None:
    """Launch the Textual UI."""
    try:
        from ui.main_window import CodebaseAnalyzerApp
    except ImportError as e:
        print(f"ERROR: Could not import UI — is 'textual' installed?")
        print(f"  pip install textual rich")
        print(f"  Detail: {e}")
        sys.exit(1)

    log.info("Launching Textual UI (demo=%s, no_chat=%s)", demo, no_chat)
    app = CodebaseAnalyzerApp()

    if no_chat:
        # Phase 1: set initial state before run
        pass

    app.run()


def run_cli(path: str) -> None:
    """
    CLI analysis mode — no UI, output to terminal.
    Phase 2 will wire this to real backend modules.
    """
    from rich.console import Console
    from rich.panel import Panel
    from rich.text import Text
    from rich import box

    console = Console()

    console.print(Panel(
        "[bold cyan]Codebase Analyzer — CLI Mode[/bold cyan]\n"
        "[dim]Phase 1: Backend not yet wired. Run Phase 2 for real analysis.[/dim]",
        box=box.ROUNDED,
        border_style="cyan",
    ))

    p = Path(path).expanduser().resolve()
    if not p.exists():
        console.print(f"[bold red]✖ Path not found:[/bold red] {p}")
        sys.exit(1)

    console.print(f"\n[bold]Target:[/bold] [cyan]{p}[/cyan]")
    console.print("[dim]Backend integration coming in Phase 2…[/dim]")
    console.print(f"\n[dim]Log file:[/dim] {get_log_path()}")


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    # Adjust log level
    import logging
    logging.getLogger("codebase_analyzer").setLevel(
        getattr(logging, args.log_level)
    )

    log.info("Starting — args: %s", args)

    if args.path:
        run_cli(args.path)
    else:
        run_ui(demo=args.demo, no_chat=args.no_chat)


if __name__ == "__main__":
    main()
