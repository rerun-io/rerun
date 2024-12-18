from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Blueprint imports

This checks that importing a blueprint into an application always applies it, regardless of its AppID.

You should be seeing a **dataframe view of a plot** on your left, instead of an _actual plot_.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_external_blueprint() -> None:
    import tempfile

    with tempfile.NamedTemporaryFile(suffix=".rbl") as tmp:
        rrb.Blueprint(
            rrb.Horizontal(
                rrb.DataframeView(
                    origin="/",
                    query=rrb.archetypes.DataframeQuery(
                        timeline="frame_nr",
                        apply_latest_at=True,
                    ),
                ),
                rrb.TextDocumentView(origin="readme"),
                column_shares=[3, 2],
            ),
        ).save("some_unrelated_blueprint_app_id", tmp.name)

        rr.log_file_from_path(tmp.name)


def log_plots() -> None:
    from math import cos, sin, tau

    def lerp(a, b, t):
        return a + t * (b - a)

    for t in range(0, int(tau * 2 * 100.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 100.0)
        rr.log(
            "trig/sin",
            rr.Scalar(sin_of_t),
            rr.SeriesLine(width=5, color=lerp(np.array([1.0, 0, 0]), np.array([1.0, 1.0, 0]), (sin_of_t + 1.0) * 0.5)),
        )

        cos_of_t = cos(float(t) / 100.0)
        rr.log(
            "trig/cos",
            rr.Scalar(cos_of_t),
            rr.SeriesLine(
                width=5, color=lerp(np.array([0.0, 1.0, 1.0]), np.array([0.0, 0.0, 1.0]), (cos_of_t + 1.0) * 0.5)
            ),
        )


def run(args: Namespace) -> None:
    rr.script_setup(
        args,
        f"{os.path.basename(__file__)}",
        recording_id=uuid4(),
    )
    rr.send_blueprint(
        rrb.Blueprint(
            rrb.Horizontal(
                rrb.TimeSeriesView(origin="/"),
                rrb.TextDocumentView(origin="readme"),
                column_shares=[3, 2],
            ),
            rrb.BlueprintPanel(state="collapsed"),
            rrb.SelectionPanel(state="collapsed"),
            rrb.TimePanel(state="collapsed"),
        ),
        make_active=True,
        make_default=True,
    )

    log_readme()
    log_plots()

    log_external_blueprint()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
