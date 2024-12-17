from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Range: partial primary and secondary updates

Checks that inter- and intra-timestamp partial updates are properly handled by range queries,
end-to-end: all the way to the views and the renderer.


You might need to de-zoom the view a bit (see [#6825](https://github.com/rerun-io/rerun/issues/6825) and
[#7281](https://github.com/rerun-io/rerun/issues/7281)).

* This entire panel should look like this:
  - ![expected](https://static.rerun.io/check_range_partial_updates_frames/4aabe76ffd5753d8054c760675121444bbefe200/768w.png)
"""


def blueprint() -> rrb.BlueprintLike:
    defaults = [
        rr.components.ColorBatch([255, 255, 0]),
        rr.components.RadiusBatch([-10]),
    ]
    return rrb.Blueprint(
        rrb.Grid(
            contents=[
                rrb.Vertical(
                    rrb.Spatial3DView(
                        name="[42:42]",
                        origin="/",
                        time_ranges=rrb.VisibleTimeRange(
                            "frame",
                            start=rrb.TimeRangeBoundary.absolute(seq=42),
                            end=rrb.TimeRangeBoundary.absolute(seq=42),
                        ),
                        defaults=defaults,
                    ),
                    rrb.Spatial3DView(
                        name="[43:44]",
                        origin="/",
                        time_ranges=rrb.VisibleTimeRange(
                            "frame",
                            start=rrb.TimeRangeBoundary.absolute(seq=43),
                            end=rrb.TimeRangeBoundary.absolute(seq=44),
                        ),
                        defaults=defaults,
                    ),
                    rrb.Spatial3DView(
                        name="[42:44]",
                        origin="/",
                        time_ranges=rrb.VisibleTimeRange(
                            "frame",
                            start=rrb.TimeRangeBoundary.absolute(seq=42),
                            end=rrb.TimeRangeBoundary.absolute(seq=44),
                        ),
                        defaults=defaults,
                    ),
                ),
                rrb.Vertical(
                    rrb.Spatial3DView(
                        name="[43:45]",
                        origin="/",
                        time_ranges=rrb.VisibleTimeRange(
                            "frame",
                            start=rrb.TimeRangeBoundary.absolute(seq=43),
                            end=rrb.TimeRangeBoundary.absolute(seq=45),
                        ),
                        defaults=defaults,
                    ),
                    rrb.Spatial3DView(
                        name="[46:46]",
                        origin="/",
                        time_ranges=rrb.VisibleTimeRange(
                            "frame",
                            start=rrb.TimeRangeBoundary.absolute(seq=46),
                            end=rrb.TimeRangeBoundary.absolute(seq=46),
                        ),
                        defaults=defaults,
                    ),
                    rrb.Spatial3DView(
                        name="[-∞:+∞]",
                        origin="/",
                        time_ranges=rrb.VisibleTimeRange(
                            "frame",
                            start=rrb.TimeRangeBoundary.infinite(),
                            end=rrb.TimeRangeBoundary.infinite(),
                        ),
                        defaults=defaults,
                    ),
                ),
                rrb.TextDocumentView(origin="readme"),
            ],
            grid_columns=3,
        ),
    )


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_points() -> None:
    rr.set_time_sequence("frame", 42)
    rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]], colors=[255, 0, 0]))

    rr.set_time_sequence("frame", 43)
    rr.log("points", [rr.components.RadiusBatch(-20)])

    rr.set_time_sequence("frame", 44)
    rr.log("points", [rr.components.ColorBatch([0, 0, 255])])

    rr.set_time_sequence("frame", 45)
    rr.log("points", rr.Points3D([[0, 0, 1], [1, 1, 0]]))
    rr.log("points", [rr.components.RadiusBatch(-40)])

    rr.set_time_sequence("frame", 46)
    rr.log("points", [rr.components.RadiusBatch(-40)])
    rr.log("points", rr.Points3D([[0, 2, 0], [1, 2, 1]]))
    rr.log("points", [rr.components.RadiusBatch(-30)])
    rr.log("points", [rr.components.ColorBatch([0, 255, 0])])
    rr.log("points", rr.Points3D([[0, 0, 2], [2, 2, 0]]))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    log_readme()
    log_points()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
