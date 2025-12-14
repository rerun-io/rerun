#!/usr/bin/env python3
"""
Recompose unified entrypoint.

This app combines all tasks and flows for the recompose project.
It serves as THE way to run recompose tasks for both development and CI.

Usage:
    uv run python examples/app.py --help
    uv run python examples/app.py lint
    uv run python examples/app.py format
    uv run python examples/app.py test
    uv run python examples/app.py ci

Inspect flows:
    uv run python examples/app.py inspect ci
"""

import sys
from pathlib import Path

# Add examples directory to path for imports
sys.path.insert(0, str(Path(__file__).parent))

# isort: off
import recompose  # noqa: E402

# Local imports - register tasks and flows with recompose
from flows import ci  # noqa: E402, F401
from tasks import format, format_check, lint, test  # noqa: E402, F401
# isort: on

# All imported tasks and flows are automatically registered
# when recompose.main() is called.

if __name__ == "__main__":
    recompose.main()
