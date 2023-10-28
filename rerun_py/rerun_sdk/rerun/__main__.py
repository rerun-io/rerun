"""See `python3 -m rerun --help`."""
from __future__ import annotations

import rerun_bindings as bindings

from rerun import unregister_shutdown


def main() -> None:
    # We don't need to call shutdown in this case. Rust should be handling everything
    # TODO(ab): we only do that because __init__.py was loaded and register_shutdown() was called.
    # Ideally, nothing should be loaded at all but `rerun_bindings` when we run `python3 -m rerun`.
    unregister_shutdown()
    exit(bindings.main())


if __name__ == "__main__":
    main()
