"""See `python3 -m rerun --help`."""

import sys

from rerun import unregister_shutdown, rerun_bindings  # type: ignore[attr-defined]

if __name__ == "__main__":
    # We don't need to call shutdown in this case. Rust should be handling everything
    unregister_shutdown()
    exit(rerun_bindings.main(sys.argv))
