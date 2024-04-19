"""
See `python3 -m rerun --help`.

This is a duplicate of `rerun_cli/__main__.py` to allow running `python3 -m rerun` directly.
In general `rerun -m rerun_cli` should be preferred, as it carries less overhead related to
importing the module.
"""

from __future__ import annotations

from rerun_cli.__main__ import main as cli_main

from rerun import unregister_shutdown


def main() -> int:
    # Importing of the rerun module registers a shutdown hook that we know we don't
    # need when running the CLI directly. We can safely unregister it.
    unregister_shutdown()

    return cli_main()


if __name__ == "__main__":
    main()
