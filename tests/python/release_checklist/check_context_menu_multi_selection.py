from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Context Menu - Multi-selection

For each of the multi-selection below, check the context menu content as per the following table.


```plaintext
==========================================================
ITEMS                               CONTEXT MENU CONTENT
==========================================================
2x Views                            Hide all
                                    Remove

                                    Expand all
                                    Collapse all

                                    Move to new container
----------------------------------------------------------
+ Vertical container                Hide all
                                    Remove

                                    Expand all
                                    Collapse all

                                    Move to new container
----------------------------------------------------------
+ Viewport                          Hide all

                                    Expand all
                                    Collapse all
==========================================================
View + 'box2d' data result          Hide all
                                    Remove

                                    Expand all
                                    Collapse all
==========================================================
'box2d' data result                 Hide all
+ 'box3d' entity (streams tree)
                                    Expand all
                                    Collapse all

                                    Add to new view
----------------------------------------------------------
+ some component                    Hide all

                                    Expand all
                                    Collapse all
==========================================================
```
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Horizontal(
        rrb.TextDocumentView(origin="readme"),
        rrb.Vertical(
            rrb.Spatial3DView(origin="/"),
            rrb.Spatial2DView(origin="/"),
        ),
        column_shares=[2, 1],
    )


def log_some_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log("boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))
    rr.log("boxes2d", rr.Boxes2D(centers=[[0, 0], [1.3, 0.5], [3, 2]], half_sizes=[0.5, 1] * 3))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())
    rr.send_blueprint(blueprint(), make_active=True, make_default=True)

    log_readme()
    log_some_views()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
