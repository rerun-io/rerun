"""Demo program which loads an rrd file built into the package."""

import argparse
import pathlib
import sys


def run_cube(args: argparse.Namespace):
    import math

    import numpy as np
    import depthai_viewer as viewer

    from rerun_demo.data import build_color_grid

    viewer.script_setup(args, "Cube")

    STEPS = 100
    twists = math.pi * np.sin(np.linspace(0, math.tau, STEPS)) / 4
    for t in range(STEPS):
        viewer.set_time_sequence("step", t)
        cube = build_color_grid(10, 10, 10, twist=twists[t])
        viewer.log_points("cube", positions=cube.positions, colors=cube.colors, radii=0.5)

    viewer.script_teardown(args)


def run_colmap(args):
    from depthai_viewer import bindings, unregister_shutdown  # type: ignore[attr-defined]

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
        print("No demo file found at {}. Package was built without demo support".format(rrd_file), file=sys.stderr)
        exit(1)
    else:
        exit(bindings.main([sys.argv[0], str(rrd_file)] + serve_opts))


def main() -> None:
    import depthai_viewer as viewer

    parser = argparse.ArgumentParser(description="Run rerun example programs.")

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

    viewer.script_add_args(parser)

    args = parser.parse_args()

    if not any([args.cube, args.colmap]):
        args.cube = True

    if args.cube:
        run_cube(args)

    elif args.colmap:
        run_colmap(args)


if __name__ == "__main__":
    main()
