from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Plot overrides

This checks whether one can override all properties in a plot.

### Component overrides

* Select `plots/cos`.
* Override all of its properties with arbitrary values.
* Remove all these overrides.

### Visible time range overrides
* Select the `plots` view and confirm it shows:
  * "Default" selected
  * Showing "Entire timeline".
* Select the `plots/cos` entity and confirm it shows:
  * "Default" selected
  * Showing "Entire timeline".
* Override the `plots` view Visible time range
  * Verify all 3 offset modes operate as expected
* Override the `plots/cos` entity Visible time range
  * Verify all 3 offset modes operate as expected

### Overrides are cloned
* After overriding things on both the view and the entity, clone the view.

If nothing weird happens, you can close this recording.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


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
