"""Create a small recording for the container-cloning before/after walkthrough."""

from pathlib import Path

import rerun as rr
import rerun.blueprint as rrb


def main() -> None:
    destination = Path(__file__).with_name("container_clone.rrd")
    rr.init("rerun_example_container_clone_7923", recording_id="container-clone-7923")
    rr.save(str(destination))
    rr.log("robot/position", rr.Points2D([[0, 0], [1, 2], [3, 1]], radii=0.15), static=True)
    rr.log(
        "notes/setup",
        rr.TextDocument(
            "# Robot dashboard\n\nThis dashboard contains a position view and a nested pair of notes tabs."
        ),
        static=True,
    )
    rr.log(
        "notes/checklist",
        rr.TextDocument(
            "# Checklist\n\n1. Clone the Robot dashboard container.\n2. Open its nested Notes tabs.\n3. Rename the copy.\n4. Confirm the original is unchanged."
        ),
        static=True,
    )
    rr.log(
        "guide",
        rr.TextDocument(
            "# Issue #7923: clone containers\n\n**Before:** individual views have Clone, but a whole container does not.\n\n**After:** clone Robot dashboard, including its layout, nested tabs, and view settings.\n\nThe copy initially shows the same recorded data. Editing its layout does not edit the original."
        ),
        static=True,
    )
    rr.send_blueprint(
        rrb.Blueprint(
            rrb.Horizontal(
                rrb.Vertical(
                    rrb.Spatial2DView(origin="robot", name="Robot positions"),
                    rrb.Tabs(
                        rrb.TextDocumentView(origin="notes/setup", name="Setup"),
                        rrb.TextDocumentView(origin="notes/checklist", name="Checklist"),
                        active_tab=1,
                        name="Notes tabs",
                    ),
                    row_shares=[2, 1],
                    name="Robot dashboard",
                ),
                rrb.TextDocumentView(origin="guide", name="Walkthrough"),
                column_shares=[3, 2],
                name="Workspace",
            ),
            collapse_panels=False,
            auto_layout=False,
            auto_views=False,
        )
    )
    print(destination)


if __name__ == "__main__":
    main()
