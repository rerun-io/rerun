from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

README = """\
# Colormap edit UI

- Click on the depth image.
- In the selection panel, check that the `Colormap` menu (in the `Visualizers` section) includes nice previews of the colormaps.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_depth_image() -> None:
    depth_image = 65535 * np.ones((200, 300), dtype=np.uint16)
    depth_image[50:150, 50:150] = 20000
    depth_image[130:180, 100:280] = 45000
    rr.log("depth", rr.DepthImage(depth_image, meter=10_000.0))


def blueprint() -> rrb.BlueprintLike:
    return rrb.Horizontal(
        rrb.TextDocumentView(origin="readme"),
        rrb.Spatial2DView(origin="/depth"),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    log_readme()
    log_depth_image()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
