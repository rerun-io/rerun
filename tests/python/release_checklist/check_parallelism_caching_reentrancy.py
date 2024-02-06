from __future__ import annotations

import math
import os
import random
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr

README = """
# Parallelism, caching, reentrancy, etc

This check simply puts a lot of pressure on all things parallel.

### Actions

TODO(cmc): simplify these instructions once we can log blueprint stuff!

* Clone the `plots` view a handful of times.
* Clone the `text_logs` view a handful of times.
* 2D Point clouds:
    * Clone the `clouds/2d` view a handful of times.
    * Edit one of the `clouds/2d` views so that it requests a visible time range of `-inf:current` instead.
    * Clone that edited `clouds/2d` view a bunch of times.
* 3D Point clouds:
    * Clone the `clouds/3d` view a handful of times.
    * Edit one of the `clouds/3d` views so that it requests a visible time range of `-inf:+int` instead.
    * Clone that edited `clouds/3d` view a bunch of times.
* Now scrub the time cursor like crazy: do your worst!

If nothing weird happens, you can close this recording.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_text_logs() -> None:
    for t in range(0, 100):
        rr.set_time_sequence("frame_nr", t)
        rr.log("text", rr.TextLog("Something good happened", level=rr.TextLogLevel.INFO))
        rr.log("text", rr.TextLog("Something bad happened", level=rr.TextLogLevel.ERROR))


def log_plots() -> None:
    from math import cos, sin, tau

    for t in range(0, int(tau * 2 * 10.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 10.0)
        rr.log("plots/sin", rr.TimeSeriesScalar(sin_of_t, label="sin(0.01t)", color=[255, 0, 0]))

        cos_of_t = cos(float(t) / 10.0)
        rr.log("plots/cos", rr.TimeSeriesScalar(cos_of_t, label="cos(0.01t)", color=[0, 255, 0]))


def log_point_clouds() -> None:
    for t in range(0, 100):
        rr.set_time_sequence("frame_nr", t)
        rr.log(
            "clouds/3d",
            rr.Points3D(
                np.array(
                    [
                        [math.sin((i + t) * 0.2) * 5, math.cos((i + t) * 0.2) * 5 - 10.0, i * 0.4 - 5.0]
                        for i in range(0, 100)
                    ]
                ),
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )
        rr.log(
            "clouds/2d",
            rr.Points2D(
                np.array(
                    [[math.sin(i * math.tau / 100.0) * t, math.cos(i * math.tau / 100.0) * t] for i in range(0, 100)]
                ),
                labels=[str(i) for i in range(t, t + 100)],
                colors=np.array([[random.randrange(255) for _ in range(3)] for _ in range(t, t + 100)]),
            ),
        )


def run(args: Namespace) -> None:
    # TODO(cmc): I have no idea why this works without specifying a `recording_id`, but
    # I'm not gonna rely on it anyway.
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_text_logs()
    log_plots()
    log_point_clouds()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
