from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Context Menu - Single Selection in the Blueprint tree

- Right-click on the viewport and "Expand All"
- Right-click on various _unselected_ items, and check that:
  - It becomes selected as the context menu appears.
  - The context menu content is as per the following table.


```plaintext
==================================================================
ITEM                       CONTEXT MENU CONTENT
==================================================================
Viewport                   Expand all
                           Collapse all

                           Add container
                           Add view
------------------------------------------------------------------
Container                  Hide (or Show, depending on visibility)
                           Remove

                           Expand all
                           Collapse all

                           Add container
                           Add view

                           Move to new container
------------------------------------------------------------------
View                       Hide (or Show, depending on visibility)
                           Remove

                           Copy screenshot
                           Save screenshot…

                           Expand all
                           Collapse all

                           Clone

                           Move to new container
------------------------------------------------------------------
'group' data result        Hide (or Show, depending on visibility)
                           Remove

                           Expand all
                           Collapse all

                           Add to new view
------------------------------------------------------------------
'boxes3d' data result      Hide (or Show, depending on visibility)
                           Remove

                           Add to new view
```

"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Horizontal(
        rrb.TextDocumentView(origin="readme"),
        rrb.Vertical(rrb.Spatial3DView(origin="/")),
    )


def log_some_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log("group/boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))


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
