from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """
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

                           Add Container
                           Add Space View
------------------------------------------------------------------
Container                  Hide (or Show, depending on visibility)
                           Remove

                           Expand all
                           Collapse all

                           Add Container
                           Add Space View

                           Move to new Container
------------------------------------------------------------------
Space View                 Hide (or Show, depending on visibility)
                           Remove

                           Expand all
                           Collapse all

                           Clone

                           Move to new Container
------------------------------------------------------------------
'group' Data Result        Hide (or Show, depending on visibility)
                           Remove

                           Expand all
                           Collapse all

                           Add to new Space View
------------------------------------------------------------------
'boxes3d' Data Result      Hide (or Show, depending on visibility)
                           Remove

                           Add to new Space View
```

"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def blueprint() -> rrb.BlueprintLike:
    return rrb.Viewport(
        rrb.Horizontal(
            rrb.TextDocumentView(origin="readme"),
            rrb.Vertical(rrb.Spatial3DView(origin="/")),
        )
    )


def log_some_space_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log("group/boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4(), blueprint=blueprint())

    log_readme()
    log_some_space_views()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
