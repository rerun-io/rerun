from __future__ import annotations

import math
import os
import random
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Parallelism, caching, reentrancy, etc

This check simply puts a lot of pressure on all things parallel.

### Actions

* Scrub the time cursor like crazy: do your worst!

If nothing weird happens, you can close this recording.
"""


def blueprint() -> rrb.BlueprintLike:
    return rrb.Grid(
        rrb.Vertical(*[rrb.TimeSeriesView(name="plots", origin="/plots") for _ in range(0, 3)]),
        rrb.Vertical(*[
            rrb.TimeSeriesView(
                name="plots",
                origin="/plots",
                time_ranges=rrb.VisibleTimeRange(
                    "frame_nr",
                    start=rrb.TimeRangeBoundary.cursor_relative(seq=50 - i * 10),
                    end=rrb.TimeRangeBoundary.cursor_relative(seq=50 - i * 10 + 10),
                ),
            )
            for i in range(0, 10)
        ]),
        rrb.Vertical(*[rrb.TextLogView(name="logs", origin="/text") for _ in range(0, 3)]),
        rrb.Vertical(*[rrb.Spatial2DView(name="2D", origin="/2D") for _ in range(0, 3)]),
        rrb.Vertical(*[
            rrb.Spatial2DView(
                name="2D",
                origin="/2D",
                time_ranges=rrb.VisibleTimeRange(
                    "frame_nr",
                    start=rrb.TimeRangeBoundary.infinite(),
                    end=rrb.TimeRangeBoundary.cursor_relative(),
                ),
            )
            for _ in range(0, 3)
        ]),
        rrb.Vertical(*[rrb.Spatial3DView(name="3D", origin="/3D") for _ in range(0, 3)]),
        rrb.Vertical(*[
            rrb.Spatial3DView(
                name="3D",
                origin="/3D",
                time_ranges=rrb.VisibleTimeRange(
                    "frame_nr",
                    start=rrb.TimeRangeBoundary.infinite(),
                    end=rrb.TimeRangeBoundary.infinite(),
                ),
            )
            for _ in range(0, 3)
        ]),
        rrb.TextDocumentView(origin="readme"),
        grid_columns=4,
    )


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_text_logs() -> None:
    for t in range(0, 100):
        rr.set_time_sequence("frame_nr", t)
        rr.log("text", rr.TextLog("Something good happened", level=rr.TextLogLevel.INFO))
        rr.log("text", rr.TextLog("Something bad happened", level=rr.TextLogLevel.ERROR))


def log_plots() -> None:
    from math import cos, sin, tau

    rr.log("plots/sin", rr.SeriesLine(color=[255, 0, 0], name="sin(0.01t)"), timeless=True)
    rr.log("plots/cos", rr.SeriesLine(color=[0, 255, 0], name="cos(0.01t)"), timeless=True)

    for t in range(0, int(tau * 2 * 10.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 10.0)
        rr.log("plots/sin", rr.Scalar(sin_of_t))

        cos_of_t = cos(float(t) / 10.0)
        rr.log("plots/cos", rr.Scalar(cos_of_t))


def log_spatial() -> None:
    for t in range(0, 100):
        rr.set_time_sequence("frame_nr", t)

        positions3d = [
            [math.sin((i + t) * 0.2) * 5, math.cos((i + t) * 0.2) * 5 - 10.0, i * 0.4 - 5.0] for i in range(0, 100)
        ]

        rr.log(
            "3D/points",
            rr.Points3D(
                np.array(positions3d),
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )
        rr.log(
            "3D/lines",
            rr.LineStrips3D(
                np.array(positions3d),
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )
        rr.log(
            "3D/arrows",
            rr.Arrows3D(
                vectors=np.array(positions3d),
                radii=0.1,
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )
        rr.log(
            "3D/boxes",
            rr.Boxes3D(
                half_sizes=np.array(positions3d),
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )

        positions2d = [[math.sin(i * math.tau / 100.0) * t, math.cos(i * math.tau / 100.0) * t] for i in range(0, 100)]

        rr.log(
            "2D/points",
            rr.Points2D(
                np.array(positions2d),
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )
        rr.log(
            "2D/lines",
            rr.LineStrips2D(
                np.array(positions2d),
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )
        rr.log(
            "2D/arrows",
            rr.Arrows2D(
                vectors=np.array(positions2d),
                radii=0.1,
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )
        rr.log(
            "2D/boxes",
            rr.Boxes2D(
                half_sizes=np.array(positions2d),
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    log_readme()
    log_text_logs()
    log_plots()
    log_spatial()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
