"""Demo program which loads an rrd file built into the package."""

import pathlib
import sys

from rerun import bindings, unregister_shutdown  # type: ignore[attr-defined]


def main() -> None:
    # We don't need to call shutdown in this case. Rust should be handling everything
    unregister_shutdown()

    rrd_file = pathlib.Path(__file__).parent.joinpath("demo.rrd").resolve()
    if not rrd_file.exists():
        print("No demo file found at {}. Package was built without demo support".format(rrd_file), file=sys.stderr)
        exit(1)
    else:
        exit(bindings.main([sys.argv[0], str(rrd_file)]))


if __name__ == "__main__":
    main()
