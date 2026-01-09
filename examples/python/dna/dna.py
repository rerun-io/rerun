#!/usr/bin/env python3
"""The example from our Getting Started page."""

from __future__ import annotations

import argparse
from math import tau

import numpy as np
import rerun as rr  # pip install rerun-sdk
from rerun import blueprint as rrb
from rerun.utilities import bounce_lerp, build_color_spiral

DESCRIPTION = """
# DNA
This is a minimal example that logs synthetic 3D data in the shape of a double helix. The underlying data is generated
using numpy and visualized using Rerun.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/dna).
""".strip()


def log_data() -> None:
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    rr.set_time("stable_time", duration=0)

    NUM_POINTS = 100

    # points and colors are both np.array((NUM_POINTS, 3))
    points1, colors1 = build_color_spiral(NUM_POINTS)
    points2, colors2 = build_color_spiral(NUM_POINTS, angular_offset=tau * 0.5)
    rr.log("helix/structure/left", rr.Points3D(points1, colors=colors1, radii=0.08), static=True)
    rr.log("helix/structure/right", rr.Points3D(points2, colors=colors2, radii=0.08), static=True)

    rr.log(
        "helix/structure/scaffolding",
        rr.LineStrips3D(np.stack((points1, points2), axis=1), colors=[128, 128, 128]),
        static=True,
    )

    time_offsets = np.random.rand(NUM_POINTS)
    for i in range(400):
        time = i * 0.01
        rr.set_time("stable_time", duration=time)

        times = np.repeat(time, NUM_POINTS) + time_offsets
        beads = [bounce_lerp(points1[n], points2[n], times[n]) for n in range(NUM_POINTS)]
        colors = [[int(bounce_lerp(80, 230, times[n] * 2))] for n in range(NUM_POINTS)]
        rr.log(
            "helix/structure/scaffolding/beads",
            rr.Points3D(beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1)),
        )

        rr.log(
            "helix/structure",
            rr.Transform3D(rotation=rr.RotationAxisAngle(axis=[0, 0, 1], radians=time / 4.0 * tau)),
        )


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    parser.add_argument(
        "--blueprint",
        action="store_true",
        help="Logs a blueprint that enables a cursor-relative time range on the beads.",
    )
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_dna_abacus")
    log_data()

    if args.blueprint:
        blueprint = rrb.Blueprint(
            rrb.Spatial3DView(
                origin="/",
                overrides={
                    "helix/structure/scaffolding/beads": rrb.VisibleTimeRanges(
                        timeline="stable_time",
                        start=rrb.TimeRangeBoundary.cursor_relative(seconds=-0.3),
                        end=rrb.TimeRangeBoundary.cursor_relative(seconds=0.3),
                    ),
                },
            ),
        )
        rr.send_blueprint(blueprint)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
