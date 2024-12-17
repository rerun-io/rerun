from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Blueprint overrides

This checks that overrides work as expected when sent via blueprint APIs.

Expected behavior:
* The `sin` plot should be a blue line (set via defaults)
* The `cos` plot should be a green points with cross markers (set via overrides)
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_plots() -> None:
    from math import cos, sin, tau

    for t in range(0, int(tau * 2 * 10.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 10.0)
        rr.log("plots/sin", rr.Scalar(sin_of_t))

        cos_of_t = cos(float(t) / 10.0)
        rr.log("plots/cos", rr.Scalar(cos_of_t))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_plots()

    blueprint = rrb.Blueprint(
        rrb.Grid(
            rrb.TextDocumentView(origin="readme", name="Instructions"),
            rrb.TimeSeriesView(
                name="Plots",
                defaults=[rr.components.Color([0, 0, 255])],
                overrides={
                    "plots/cos": [
                        rrb.VisualizerOverrides("SeriesPoint"),
                        rr.components.Color([0, 255, 0]),
                        # TODDO(#6670): This should just be `rr.components.MarkerShape.Cross`
                        rr.components.MarkerShapeBatch("cross"),
                    ],
                },
            ),
        )
    )
    rr.send_blueprint(blueprint, make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
