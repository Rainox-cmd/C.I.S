"""
utils/file_handler.py
Handles all input methods: folder path, ZIP extraction, path normalization.
UI calls these functions — backend never handles raw input directly.
"""

import os
import shutil
import zipfile
import tempfile
from pathlib import Path
from typing import Optional

from utils.logger import get_logger

log = get_logger(__name__)


class FileHandlerError(Exception):
    """Raised when input cannot be prepared for scanning."""


def resolve_project_path(raw_input: str) -> Path:
    """
    Accept either a folder path or a ZIP file path.
    Returns a resolved, validated directory Path ready for scanning.
    
    - Folder path  → validated and returned as-is
    - ZIP path     → extracted to a temp dir, returns that dir
    """
    p = Path(raw_input.strip()).expanduser().resolve()

    if not p.exists():
        raise FileHandlerError(f"Path does not exist: {p}")

    if p.is_dir():
        log.info("Resolved folder input: %s", p)
        return p

    if p.is_file() and p.suffix.lower() == ".zip":
        log.info("ZIP input detected: %s", p)
        return _extract_zip(p)

    raise FileHandlerError(
        f"Input must be a folder or a .zip file. Got: {p}"
    )


def _extract_zip(zip_path: Path) -> Path:
    """
    Safely extract ZIP to a fresh temp directory.
    Returns the extracted root directory.
    Guards against path traversal attacks (zip slip).
    """
    extract_root = Path(tempfile.mkdtemp(prefix="cba_zip_"))
    log.info("Extracting ZIP to: %s", extract_root)

    with zipfile.ZipFile(zip_path, "r") as zf:
        for member in zf.namelist():
            # Guard: reject any path that escapes the extract root
            member_path = (extract_root / member).resolve()
            if not str(member_path).startswith(str(extract_root)):
                log.warning("Skipping unsafe ZIP entry: %s", member)
                continue
            zf.extract(member, extract_root)

    # If ZIP contains a single top-level folder, descend into it
    entries = list(extract_root.iterdir())
    if len(entries) == 1 and entries[0].is_dir():
        log.debug("Single top-level folder detected, descending: %s", entries[0])
        return entries[0]

    return extract_root


def cleanup_temp_dir(path: Path) -> None:
    """
    Remove a temp directory created during ZIP extraction.
    Call this when analysis is complete to free disk space.
    """
    if path.exists() and str(path).startswith(tempfile.gettempdir()):
        shutil.rmtree(path, ignore_errors=True)
        log.info("Cleaned up temp dir: %s", path)


def normalize_relative_path(root: Path, full_path: Path) -> str:
    """Return a clean relative path string for display."""
    try:
        return str(full_path.relative_to(root))
    except ValueError:
        return str(full_path)


def is_valid_project_root(path: Path) -> bool:
    """Quick sanity check before launching a scan."""
    return path.exists() and path.is_dir() and any(path.iterdir())
