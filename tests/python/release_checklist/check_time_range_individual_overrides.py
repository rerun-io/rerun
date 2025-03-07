from __future__ import annotations

import os
from argparse import Namespace
from math import tau
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb
from rerun.utilities import bounce_lerp, build_color_spiral

README = """\
# Individual time-range overrides

You should see the familiar Helix example, but the beads (and the beads only), should have a cursor-relative
time-range applied to them.

If you select the beads, you should see a `Visible Time Range` set to `[-0.3, +0.3]`.
"""

EXPECTED = """\
It should look something like this:
* ![expected](https://static.rerun.io/individual_overrides/7a9813500b81c2eeaf2a86f3603dab88bfd6b8c1/480w.png)
"""


def blueprint() -> rrb.BlueprintLike:
    return rrb.Blueprint(
        rrb.Horizontal(
            contents=[
                rrb.Spatial3DView(
                    origin="/",
                    overrides={
                        "helix/structure/scaffolding/beads": rrb.VisibleTimeRanges(
                            rrb.VisibleTimeRange(
                                timelines="stable_time",
                                starts=rrb.TimeRangeBoundary.cursor_relative(seconds=-0.3),
                                ends=rrb.TimeRangeBoundary.cursor_relative(seconds=0.3),
                            ),
                        ),
                    },
                ),
                rrb.Vertical(
                    rrb.TextDocumentView(origin="readme"),
                    rrb.TextDocumentView(origin="expected"),
                    row_shares=[1, 3],
                ),
            ],
        ),
    )


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_helix() -> None:
    rr.set_index("stable_time", timedelta=0)

    rr.log(
        "expected",
        rr.TextDocument(EXPECTED, media_type=rr.MediaType.MARKDOWN),
    )

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
        rr.set_index("stable_time", timedelta=time)

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


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    rr.send_blueprint(blueprint(), make_default=True, make_active=True)

    log_readme()
    log_helix()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
