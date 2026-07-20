"""Development helper for rerun-sdk editable installs."""

from __future__ import annotations

import os
import sys
from pathlib import Path


def _find_repo_root() -> Path | None:
    """Find the rerun repo root directory."""
    # Try PIXI_PROJECT_ROOT first (set when running under pixi)
    pixi_root = os.environ.get("PIXI_PROJECT_ROOT")
    if pixi_root:
        return Path(pixi_root)

    # Otherwise walk up from the venv (sys.prefix) to find the repo root. For the
    # workspace .venv this is the immediate parent; for an isolated example's .venv
    # (examples/python/<name>/.venv) it is several levels up. The repo root is the
    # first ancestor that holds both `pixi.toml` and the `rerun_py` source tree.
    for parent in Path(sys.prefix).parents:
        if (parent / "pixi.toml").is_file() and (parent / "rerun_py").is_dir():
            return parent

    return None


def init() -> None:
    """
    Sitecustomize entrypoint that sets RERUN_CLI_PATH.

    This runs early during Python startup (before .pth files),
    ensuring the env var is set before rerun_sdk is imported.
    """
    # Don't override if already set
    if "RERUN_CLI_PATH" in os.environ:
        return

    repo_root = _find_repo_root()
    if repo_root is None:
        return

    cli_path = repo_root / "target" / "debug" / "rerun"

    if cli_path.exists():
        os.environ["RERUN_CLI_PATH"] = str(cli_path)
