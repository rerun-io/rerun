"""Demo program which loads an rrd file built into the package."""
from __future__ import annotations

import argparse
import pathlib
import sys


def run_cube(args: argparse.Namespace):
    import math

    import numpy as np
    import rerun as rr

    from rerun_demo.data import build_color_grid

    rr.script_setup(args, "Cube")

    STEPS = 100
    twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
    for t in range(STEPS):
        rr.set_time_sequence("step", t)
        cube = build_color_grid(10, 10, 10, twist=twists[t])
        rr.log_points("cube", positions=cube.positions, colors=cube.colors, radii=0.5)

    rr.script_teardown(args)


def run_structure_from_motion(args):
    from rerun import bindings, unregister_shutdown  # type: ignore[attr-defined]

    serve_opts = []

    # TODO(https://github.com/rerun-io/rerun/issues/1924): The need to special-case
    # this flag conversion is a bit awkward.
    if args.connect or args.addr:
        print("Connecting to external viewer is only supported with the --cube demo.", file=sys.stderr)
        exit(1)
    if args.save:
        print("Saving an RRD file is only supported from the --cube demo.", file=sys.stderr)
        exit(1)
    if args.serve:
        serve_opts.append("--web-viewer")

    # We don't need to call shutdown in this case. Rust should be handling everything
    unregister_shutdown()

    rrd_file = pathlib.Path(__file__).parent.joinpath("colmap_fiat.rrd").resolve()
    if not rrd_file.exists():
        print(f"No demo file found at {rrd_file}. Package was built without demo support", file=sys.stderr)
        exit(1)
    else:
        exit(bindings.main([sys.argv[0], str(rrd_file)] + serve_opts))


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
