"""See `python3 -m rerun --help`."""

from __future__ import annotations

import os
import subprocess
import sys


def main() -> int:
    if "RERUN_CLI_PATH" in os.environ:
        print(f"Using overridden RERUN_CLI_PATH={os.environ['RERUN_CLI_PATH']}", file=sys.stderr)
        target_path = os.environ["RERUN_CLI_PATH"]
    else:
        target_path = os.path.join(os.path.dirname(__file__), "..", "bin", "rerun")

    return subprocess.call([target_path, *sys.argv[1:]])


if __name__ == "__main__":
    main()
