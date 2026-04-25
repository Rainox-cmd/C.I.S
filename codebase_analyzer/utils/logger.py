"""
utils/logger.py
Centralized logger for the Codebase Analyzer.
All modules import from here — never configure logging elsewhere.
"""

import logging
import sys
from pathlib import Path
from datetime import datetime

LOG_DIR = Path.home() / ".codebase_analyzer" / "logs"
LOG_DIR.mkdir(parents=True, exist_ok=True)

_LOG_FILE = LOG_DIR / f"run_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log"

# ─── Formatter ────────────────────────────────────────────────────────────────

_fmt = logging.Formatter(
    fmt="%(asctime)s | %(levelname)-8s | %(name)-25s | %(message)s",
    datefmt="%H:%M:%S",
)

# ─── Handlers ─────────────────────────────────────────────────────────────────

_file_handler = logging.FileHandler(_LOG_FILE, encoding="utf-8")
_file_handler.setFormatter(_fmt)
_file_handler.setLevel(logging.DEBUG)

_console_handler = logging.StreamHandler(sys.stderr)
_console_handler.setFormatter(_fmt)
_console_handler.setLevel(logging.WARNING)   # Only warnings+ to stderr

# ─── Root setup ───────────────────────────────────────────────────────────────

_root = logging.getLogger("codebase_analyzer")
_root.setLevel(logging.DEBUG)
_root.addHandler(_file_handler)
_root.addHandler(_console_handler)
_root.propagate = False


def get_logger(name: str) -> logging.Logger:
    """
    Return a child logger.
    Usage:
        from utils.logger import get_logger
        log = get_logger(__name__)
        log.info("Scanning started")
    """
    return _root.getChild(name)


def get_log_path() -> Path:
    return _LOG_FILE
