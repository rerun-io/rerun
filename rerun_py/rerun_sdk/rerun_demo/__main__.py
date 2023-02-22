"""Demo program which loads an rrd file built into the package."""

import argparse
import pathlib
import sys


def run_cube():
    import math

    import numpy as np
    import rerun as rr

    rr.init("Cube", spawn=True, default_enabled=True)
    from rerun_demo.data import build_color_grid

    STEPS = 100
    twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
    for t in range(STEPS):
        rr.set_time_sequence("step", t)
        cube = build_color_grid(10, 10, 10, twist=twists[t])
        rr.log_points("cube", positions=cube.positions, colors=cube.colors, radii=0.5)


def run_colmap():
    from rerun import bindings, unregister_shutdown  # type: ignore[attr-defined]

    # We don't need to call shutdown in this case. Rust should be handling everything
    unregister_shutdown()

    rrd_file = pathlib.Path(__file__).parent.joinpath("colmap.rrd").resolve()
    if not rrd_file.exists():
        print("No demo file found at {}. Package was built without demo support".format(rrd_file), file=sys.stderr)
        exit(1)
    else:
        exit(bindings.main([sys.argv[0], str(rrd_file)]))


def main() -> None:
    parser = argparse.ArgumentParser(description="Run rerun example programs")

    group = parser.add_mutually_exclusive_group()

    group.add_argument(
        "--cube",
        action="store_true",
        help="Run the color grid cube demo",
    )

    group.add_argument(
        "--colmap",
        action="store_true",
        help="Run the COLMAP data demo",
    )

    args = parser.parse_args()

    if not any([args.cube, args.colmap]):
        args.cube = True

    if args.cube:
        run_cube()

    elif args.colmap:
        run_colmap()


if __name__ == "__main__":
    main()
