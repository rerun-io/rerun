from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """
# Plot overrides

This checks whether one can override all properties in a plot.

### Component overrides

* Select `plots/cos`.
* Override all of its properties with arbitrary values.
* Remove all these overrides.

### Visible time range overrides
* Select the `plots` space view and confirm it shows:
  * "Default" selected
  * Showing data between frames -∞ and +∞ (included).
* Select the `plots/cos` entity and confirm it shows:
  * "Default" selected
  * Showing data between frames -∞ and +∞ (included).
* Override the `plots` space view Visible time range
  * Verify all 3 offset modes operate as expected
* Override the `plots/cos` entity Visible time range
  * Verify all 3 offset modes operate as expected

### Overrides are cloned
* After overriding things on both the space-view and the entity, clone the space-view.

If nothing weird happens, you can close this recording.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_plots() -> None:
    from math import cos, sin, tau

    for t in range(0, int(tau * 2 * 10.0)):
        rr.set_time_sequence("frame_nr", t)

        sin_of_t = sin(float(t) / 10.0)
        rr.log("plots/sin", rr.TimeSeriesScalar(sin_of_t, label="sin(0.01t)", color=[255, 0, 0]))

        cos_of_t = cos(float(t) / 10.0)
        rr.log("plots/cos", rr.TimeSeriesScalar(cos_of_t, label="cos(0.01t)", color=[0, 255, 0]))


def run(args: Namespace) -> None:
    # TODO(cmc): I have no idea why this works without specifying a `recording_id`, but
    # I'm not gonna rely on it anyway.
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_plots()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
