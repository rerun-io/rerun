from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Scalar clears

This checks whether scalar time series correctly behave with `Clear`s.

### Actions

Look at the plot, you should see a big hole in the middle, where `Clear`s were logged.
If so, you can close this recording.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_plots() -> None:
    from math import sin, tau

    rr.log("plots/line", rr.SeriesLine(), static=True)
    rr.log("plots/point", rr.SeriesPoint(), static=True)

    for t in range(0, int(tau * 2 * 10.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 10.0)

        if t > 30 and t < 90:
            rr.log("plots", rr.Clear(recursive=True))
        else:
            rr.log("plots/line", rr.Scalar(sin_of_t))
            rr.log("plots/point", rr.Scalar(sin_of_t))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_plots()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
