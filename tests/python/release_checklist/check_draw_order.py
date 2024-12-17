from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# 2D Draw order

This checks whether the draw order correctly determines the layering of 2D content.

### Action
You should see a single 2D view with the following elements overdrawing each other back to front:
- Gray background image
- Green to red gradient image
- Blue square
- Red line strip (zig-zagging)
- Black Rectangle (Box2D)
- Raster of points (visibly overlaps the rectangle!)
- White square
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def run_2d_layering() -> None:
    rr.set_time_seconds("sim_time", 1)

    # Large gray background.
    img = np.full((256, 512), 64, dtype="uint8")
    rr.log("2d_layering/background", rr.Image(img, draw_order=0.0))

    # Smaller gradient in the middle.
    img = np.zeros((256, 256, 3), dtype="uint8")
    img[:, :, 0] = np.linspace(0, 255, 256, dtype="uint8")
    img[:, :, 1] = np.linspace(0, 255, 256, dtype="uint8")
    img[:, :, 1] = img[:, :, 1].transpose()
    rr.log("2d_layering/middle_gradient", rr.Image(img, draw_order=1.0))

    # Slightly smaller blue in the middle, on the same layer as the previous.
    img = np.full((192, 192, 3), (0, 0, 255), dtype="uint8")
    rr.log("2d_layering/middle_blue", rr.Image(img, draw_order=1.1))

    rr.log(
        "2d_layering/lines_behind_rect",
        rr.LineStrips2D([(i * 20, i % 2 * 100 + 70) for i in range(20)], draw_order=1.25, colors=0xFF0000FF),
    )

    rr.log(
        "2d_layering/rect_between_top_and_middle",
        rr.Boxes2D(array=[64, 32, 256, 128], draw_order=1.5, array_format=rr.Box2DFormat.XYWH, colors=0),
    )

    rr.log(
        "2d_layering/points_between_top_and_middle",
        rr.Points2D(
            [(32.0 + int(i / 16) * 16.0, 32.0 + (i % 16) * 16.0) for i in range(16 * 16)],
            draw_order=1.51,
        ),
    )

    # Small white square on top.
    img = np.full((128, 128), 255, dtype="uint8")
    rr.log("2d_layering/top", rr.Image(img, draw_order=2.0))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(
        rrb.Blueprint(rrb.Grid(rrb.Spatial2DView(origin="/"), rrb.TextDocumentView(origin="readme"))),
        make_active=True,
        make_default=True,
    )

    log_readme()
    run_2d_layering()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
