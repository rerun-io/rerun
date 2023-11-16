"""Demo program which loads an rrd file built into the package."""
from __future__ import annotations

import argparse


def run_cube(args: argparse.Namespace):
    import math

    import numpy as np
    import rerun as rr

    from rerun_demo.data import build_color_grid

    rr.script_setup(args, "rerun_example_cube")

    STEPS = 100
    twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
    for t in range(STEPS):
        rr.set_time_sequence("step", t)
        cube = build_color_grid(10, 10, 10, twist=twists[t])
        rr.log("cube", rr.Points3D(positions=cube.positions, colors=cube.colors, radii=0.5))

    rr.script_teardown(args)


def run_structure_from_motion(args):
    print(
        "`run_structure_from_motion` has been deprecated. You can execute `rerun`, and use the built-in examples instead."
    )


def main() -> None:
    import rerun as rr

    parser = argparse.ArgumentParser(description="Run rerun example programs.")

    group = parser.add_mutually_exclusive_group()

    group.add_argument(
        "--cube",
        action="store_true",
        help="Run the color grid cube demo",
    )

    group.add_argument(
        "--structure-from-motion",
        action="store_true",
        help="Run the COLMAP data demo",
    )

    rr.script_add_args(parser)

    args = parser.parse_args()

    if not any([args.cube, args.structure_from_motion]):
        args.cube = True

    if args.cube:
        run_cube(args)

    elif args.structure_from_motion:
        run_structure_from_motion(args)


if __name__ == "__main__":
    main()
