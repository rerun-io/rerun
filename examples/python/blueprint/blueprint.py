#!/usr/bin/env python3
"""Example of using the blueprint APIs to configure Rerun."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb


def main() -> None:
    parser = argparse.ArgumentParser(description="Different options for how we might use blueprint")

    parser.add_argument("--skip-blueprint", action="store_true", help="Don't send the blueprint")
    parser.add_argument("--auto-space-views", action="store_true", help="Automatically add space views")

    args = parser.parse_args()

    if args.skip_blueprint:
        blueprint = None
    else:
        # Create a blueprint which includes 2 additional views each only showing 1 of the two
        # rectangles.
        #
        # If auto_space_views is True, the blueprint will automatically add one of the heuristic
        # space views, which will include the image and both rectangles.
        blueprint = rrb.Blueprint(
            rrb.Grid(
                rrb.Spatial2DView(name="Rect 0", origin="/", contents=["image", "rect/0"]),
                rrb.Spatial2DView(
                    name="Rect 1",
                    origin="/",
                    contents=["/**"],
                    defaults=[rr.components.Radius(2)],  # Default all rectangles to have a radius of 2
                    overrides={"rect/0": [rr.components.Radius(1)]},  # Override the radius of rect/0 to be 1
                ),
            ),
            rrb.BlueprintPanel(state="collapsed"),
            rrb.SelectionPanel(state="collapsed"),
            rrb.TimePanel(state="collapsed"),
            auto_space_views=args.auto_space_views,
        )

    rr.init("rerun_example_blueprint", spawn=True, default_blueprint=blueprint)

    img = np.zeros([128, 128, 3], dtype="uint8")
    for i in range(8):
        img[(i * 16) + 4 : (i * 16) + 12, :] = (0, 0, 200)
    rr.log("image", rr.Image(img))
    rr.log(
        "rect/0",
        rr.Boxes2D(mins=[16, 16], sizes=[64, 64], labels="Rect0", colors=(255, 0, 0)),
    )
    rr.log(
        "rect/1",
        rr.Boxes2D(mins=[48, 48], sizes=[64, 64], labels="Rect1", colors=(0, 255, 0)),
    )


if __name__ == "__main__":
    main()
