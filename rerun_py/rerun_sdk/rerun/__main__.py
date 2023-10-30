"""See `python3 -m rerun --help`."""
from __future__ import annotations

import rerun_bindings as bindings

from rerun import unregister_shutdown


def main() -> None:
    # When running `python -m rerun` (i.e. executing this file), the `rerun` package and its `__init__.py` are still
    # loaded. This has the side effect of executing `register_shutdown()` (schedule `bindings.shutdown()` to be called
    # at exit. We don't need this here, so we unregister that call.
    # TODO(ab): figure out a way to skip loading `__init__.py` entirely (to avoid doing this and save on loading time)
    unregister_shutdown()

    exit(bindings.main())


if __name__ == "__main__":
    main()
