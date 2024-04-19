"""
See `python3 -m rerun --help`.

This is a duplicate of `rerun_cli/__main__.py` to allow running `python3 -m rerun` directly.
In general `rerun -m rerun_cli` should be preferred, as it carries less overhead related to
importing the module.
"""

from __future__ import annotations

import os
import subprocess
import sys

from rerun import unregister_shutdown


def exe_suffix() -> str:
    if sys.platform.startswith("win"):
        return ".exe"
    return ""


def add_exe_suffix(path: str) -> str:
    if not path.endswith(exe_suffix()):
        return path + exe_suffix()
    return path


def main() -> int:
    # Importing of the rerun module registers a shutdown hook that we know we don't
    # need when running the CLI directly. We can safely unregister it.
    unregister_shutdown()
    if "RERUN_CLI_PATH" in os.environ:
        print(f"Using overridden RERUN_CLI_PATH={os.environ['RERUN_CLI_PATH']}", file=sys.stderr)
        target_path = os.environ["RERUN_CLI_PATH"]
    else:
        target_path = os.path.join(os.path.dirname(__file__), "..", "rerun_cli", "rerun")

    target_path = add_exe_suffix(target_path)

    if not os.path.exists(target_path):
        print(f"Error: Could not find rerun binary at {target_path}", file=sys.stderr)
        return 1

    return subprocess.call([target_path, *sys.argv[1:]])


if __name__ == "__main__":
    main()
