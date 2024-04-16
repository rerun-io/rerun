"""See `python3 -m rerun --help`."""

from __future__ import annotations

import os
import subprocess
import sys


def main() -> int:
    return subprocess.call([os.path.join(os.path.dirname(__file__), "..", "bin", "rerun"), *sys.argv[1:]])


if __name__ == "__main__":
    main()
