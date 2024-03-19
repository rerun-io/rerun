#!/usr/bin/env python3
"""Example of using the blueprint APIs to configure Rerun."""
from __future__ import annotations

import argparse

import numpy as np
import rerun as rr  # pip install rerun-sdk
from rerun.blueprint import Blueprint, BlueprintPanel, Grid, SelectionPanel, Spatial2DView, TimePanel, Viewport


def main() -> None:
    parser = argparse.ArgumentParser(description="Different options for how to use blueprint")

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
        blueprint = Blueprint(
            Viewport(
                Grid(
                    Spatial2DView(name="Rect 0", origin="/", contents=["image", "rect/0"]),
                    Spatial2DView(name="Rect 1", origin="/", contents=["image", "rect/1"]),
                ),
                auto_space_views=args.auto_space_views,
            ),
            BlueprintPanel(expanded=False),
            SelectionPanel(expanded=False),
            TimePanel(expanded=False),
        )

    rr.init("rerun_example_blueprint", spawn=True, blueprint=blueprint)
    img = np.array([[(147, 204, 234), (9, 31, 146)], [(9, 31, 146), (147, 204, 234)]], dtype=np.uint8)
    img = np.tile(img, (8, 8, 1))

    rr.log("image", rr.Image(img))
    rr.log("rect/0", rr.Boxes2D(mins=[1, 1], sizes=[4, 4], labels="Rect0", colors=(255, 0, 0)))
    rr.log("rect/1", rr.Boxes2D(mins=[6, 6], sizes=[4, 4], labels="Rect1", colors=(0, 255, 0)))


if __name__ == "__main__":
    main()
