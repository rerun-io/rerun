"""See `python3 -m rerun --help`."""

import sys

from rerun import rerun_bindings  # type: ignore[attr-defined]

if __name__ == "__main__":
    rerun_bindings.main(sys.argv)
