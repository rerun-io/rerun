from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr

README = """\
# 1D Image/Tensor/BarChart

This checks the different ways 1D arrays can be visualized.

### Actions

You should see:
* a tensor view with 1D data
  * Note: when selecting the tensor view, there should be two "Dimension Mapping" widgets, which can be used to
    display the tensor vertically or horizontally. The "Selectors" list should be empty.
* an image view with a 1D image
* a bar chart

Known bugs:
* TODO(#6695): When hovering over a the tensor view, a thin, black, rounded cutout appears.

Bonus actions:
* use the ui to create a tensor/bar-chart with each of the entities no matter how it was logged
    * TODO(#5847): Right now tensors & bar charts can not be reinterpreted as 2D images.
      In this example, image is correctly not suggested for the `tensor` and `image` entities,
      since they are of 1D shape, but this would be relevant if they were 1xN or Nx1.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_1d_data() -> None:
    x = np.linspace(0.0, 100.0, 100)
    rr.log("tensor", rr.Tensor(x))
    rr.log("barchart", rr.BarChart(x))
    # We're not allowing "real" 1D here and force users to be explicit about width/height
    rr.log("image", rr.Image(np.reshape(x, (1, 100))))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_1d_data()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
