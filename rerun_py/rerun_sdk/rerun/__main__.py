"""See `python3 -m rerun --help`."""

import sys

from rerun_sdk import rerun_bindings
from rerun_sdk.rerun import unregister_shutdown  # type: ignore[attr-defined]


def main() -> None:
    # We don't need to call shutdown in this case. Rust should be handling everything
    unregister_shutdown()
    exit(rerun_bindings.main(sys.argv))


if __name__ == "__main__":
    main()
