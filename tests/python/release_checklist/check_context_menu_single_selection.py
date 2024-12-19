from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Context Menu - Single Selection

#### Streams tree

- Right-click on various _unselected_ items, and check that:
  - It becomes selected as the context menu appears.
  - The context menu content is as per the following table.


```plaintext
=================================================
ITEM                      CONTEXT MENU CONTENT
=================================================
'group/' entity           Expand all
                          Collapse all

                          Add to new view
-------------------------------------------------
Component                 <no action available>
=================================================
```

#### Tile title UI

- Multi-select the 3D view and the Vertical container in the Blueprint tree.
- Right-click on the 3D view tab title:
  - The selection is set to the view _only_.
  - The context menu content is as per the following table.


```plaintext
=================================================
ITEM                      CONTEXT MENU CONTENT
=================================================
view (tab title)          Hide
                          Remove

                          Copy screenshot
                          Save screenshot…

                          Expand all
                          Collapse all

                          Clone

                          Move to new container
=================================================
```


#### Container selection panel child list

- Select the Vertical container.
- In the selection panel, right-click on the 3D view, and check that:
  - The selection remains unchanged.
  - The context menu content is as per the following table.

```plaintext
=================================================
ITEM                      CONTEXT MENU CONTENT
=================================================
view (child list)         Hide
                          Remove

                          Copy screenshot
                          Save screenshot…

                          Expand all
                          Collapse all

                          Clone

                          Move to new Container
=================================================
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
