from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Latest-at: partial primary and secondary updates

Checks that inter- and intra-timestamp partial updates are properly handled by latest-at queries,
end-to-end: all the way to the views and the renderer.

For each frame, compare the view on the left with the expectation shown below.

You might need to de-zoom a bit on each view (see [#6825](https://github.com/rerun-io/rerun/issues/6825) and
[#7281](https://github.com/rerun-io/rerun/issues/7281)).
"""

FRAME_42 = """\
Frame #42 should look like this:
* ![expected](https://static.rerun.io/check_latest_at_partial_updates_frame42/3ed69ef182d8e475a36fd9351669942f5092859f/480w.png)
"""

FRAME_43 = """\
Frame #43 should look like this:
* ![expected](https://static.rerun.io/check_latest_at_partial_updates_frame43/e86013ac21cc3b6bc17aceecc7cbb9e454128150/480w.png)
"""

FRAME_44 = """\
Frame #44 should look like this:
* ![expected](https://static.rerun.io/check_latest_at_partial_updates_frame44/df5d4bfe74bcf5fc12ad658f62f35908ceff80bf/480w.png)
"""

FRAME_45 = """\
Frame #45 should look like this:
* ![expected](https://static.rerun.io/check_latest_at_partial_updates_frame45/8c19fcbe9b7c59ed9e27452a5d2696eee84a4a55/480w.png)
"""

FRAME_46 = """\
Frame #46 should look like this:
* ![expected](https://static.rerun.io/check_latest_at_partial_updates_frame46/a7f7d8f5b07c1e3fe4ff66e42fd473d2f2edb04b/480w.png)
"""


def blueprint() -> rrb.BlueprintLike:
    # TODO(#6825, #7281): set the camera properly so users don't have to manually de-zoom.
    return rrb.Blueprint(
        rrb.Horizontal(
            contents=[
                rrb.Spatial3DView(
                    name="3D",
                    origin="/",
                    defaults=[
                        rr.components.ColorBatch([255, 255, 0]),
                        rr.components.RadiusBatch([-10]),
                    ],
                ),
                rrb.Vertical(
                    rrb.TextDocumentView(origin="readme"),
                    rrb.TextDocumentView(origin="expected"),
                    row_shares=[1, 3],
                ),
            ]
        ),
    )


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)


def log_points() -> None:
    rr.set_time_sequence("frame", 42)
    rr.log(
        "expected",
        rr.TextDocument(FRAME_42, media_type=rr.MediaType.MARKDOWN),
    )
    rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))

    rr.set_time_sequence("frame", 43)
    rr.log(
        "expected",
        rr.TextDocument(FRAME_43, media_type=rr.MediaType.MARKDOWN),
    )
    rr.log("points", [rr.components.RadiusBatch(-20)])

    rr.set_time_sequence("frame", 44)
    rr.log(
        "expected",
        rr.TextDocument(FRAME_44, media_type=rr.MediaType.MARKDOWN),
    )
    rr.log("points", [rr.components.ColorBatch([0, 0, 255])])

    rr.set_time_sequence("frame", 45)
    rr.log(
        "expected",
        rr.TextDocument(FRAME_45, media_type=rr.MediaType.MARKDOWN),
    )
    rr.log("points", rr.Points3D([[0, 0, 1], [1, 1, 0]]))

    rr.set_time_sequence("frame", 46)
    rr.log(
        "expected",
        rr.TextDocument(FRAME_46, media_type=rr.MediaType.MARKDOWN),
    )
    rr.log("points", rr.Points3D([[0, 0, 0], [1, 1, 1]]))
    rr.log("points", [rr.components.RadiusBatch(-30)])
    rr.log("points", [rr.components.ColorBatch([0, 255, 0])])
    rr.log("points", rr.Points3D([[0, 0, 1], [1, 1, 0]]))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    rr.send_blueprint(blueprint(), make_default=True, make_active=True)

    log_readme()
    log_points()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
