"""See `python3 -m rerun --help`."""

import sys

from rerun import rerun_sdk  # type: ignore[attr-defined]

if __name__ == "__main__":
    rerun_sdk.main(sys.argv)
