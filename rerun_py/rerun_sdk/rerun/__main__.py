"""See `python3 -m rerun --help`."""
from __future__ import annotations

import sys

from rerun import bindings, unregister_shutdown


def main() -> None:
    # We don't need to call shutdown in this case. Rust should be handling everything
    unregister_shutdown()
    exit(bindings.main(sys.argv))


if __name__ == "__main__":
    main()
