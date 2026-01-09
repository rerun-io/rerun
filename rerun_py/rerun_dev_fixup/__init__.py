"""Development helper for rerun-sdk editable installs."""

from __future__ import annotations

import os
from pathlib import Path


def init() -> None:
    """
    Sitecustomize entrypoint that sets RERUN_CLI_PATH.

    This runs early during Python startup (before .pth files),
    ensuring the env var is set before rerun_sdk is imported.
    """
    # Don't override if already set
    if "RERUN_CLI_PATH" in os.environ:
        return

    # This file is at: rerun_py/rerun_dev_fixup/__init__.py
    # We want:         target/debug/rerun
    repo_root = Path(__file__).parent.parent.parent
    cli_path = repo_root / "target" / "debug" / "rerun"

    if cli_path.exists():
        os.environ["RERUN_CLI_PATH"] = str(cli_path)
