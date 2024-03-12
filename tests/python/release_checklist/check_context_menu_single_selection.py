from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """
# Context Menu - Single Selection

## Preparation

TODO(ab): automate this with blueprints

- Reset the blueprint
- Add a Horizontal container in the viewport and move the 3D space view into it
- Right-click on the viewport and "Expand All"


## Streams tree

- Right-click on various _unselected_ items, and check that:
  - It becomes selected as the context menu appears.
  - The context menu content is as per the following table.


```plaintext
ITEM                                CONTEXT MENU CONTENT


'group/' entity                     Expand all
                                    Collapse all
                                    Add to new Space View


Component                           <no action available>

```

## Tile Title UI

- Multi-select the 3D space view and the Horizontal container in the Blueprint tree.
- Right-click on the 3D space view tab title:
  - The selection is set to the space view _only_.
  - The context menu content is as per the following table.


```plaintext
ITEM                                CONTEXT MENU CONTENT


space view (tab title)              Hide (or Show, depending on visibility)d
                                    Remove
                                    Expand all
                                    Collapse all
                                    Clone
                                    Move to new Container

```


## Container Selection Panel child list

- Select the Horizontal container.
- In the selection panel, right-click on the 3D space view, and check that:
  - The selection remains unchanged.
  - The context menu content is as per the following table.

```plaintext
ITEM                                CONTEXT MENU CONTENT


space view (child list)             Hide (or Show, depending on visibility)
                                    Remove
                                    Expand all
                                    Collapse all
                                    Clone
                                    Move to new Container

```

"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_some_space_views() -> None:
    rr.set_time_sequence("frame_nr", 0)

    rr.log("group/boxes3d", rr.Boxes3D(centers=[[0, 0, 0], [1, 1.5, 1.15], [3, 2, 1]], half_sizes=[0.5, 1, 0.5] * 3))


def run(args: Namespace) -> None:
    # TODO(cmc): I have no idea why this works without specifying a `recording_id`, but
    # I'm not gonna rely on it anyway.
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_some_space_views()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
