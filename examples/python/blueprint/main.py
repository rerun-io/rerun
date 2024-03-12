#!/usr/bin/env python3
"""Example of using the blueprint APIs to configure Rerun."""
# TODO(jleibs): Update this example to use the new APIs
from __future__ import annotations

import argparse

import numpy as np
import rerun as rr  # pip install rerun-sdk
import rerun.experimental as rr_exp


def main() -> None:
    parser = argparse.ArgumentParser(description="Different options for how we might use blueprint")

    parser.add_argument("--blueprint-only", action="store_true", help="Only send the blueprint")
    parser.add_argument("--skip-blueprint", action="store_true", help="Don't send the blueprint")
    parser.add_argument(
        "--no-append-default", action="store_false", help="Append to the default blueprint instead of replacing it"
    )
    parser.add_argument("--auto-space-views", action="store_true", help="Automatically add space views")

    args = parser.parse_args()

    if args.blueprint_only:
        # If only using blueprint, it's important to specify init_logging=False
        rr.init(
            "Blueprint demo",
            init_logging=False,
            spawn=True,
        )
    else:
        rr.init(
            "Blueprint demo",
            spawn=True,
        )

    if not args.blueprint_only:
        img = np.zeros([128, 128, 3], dtype="uint8")
        for i in range(8):
            img[(i * 16) + 4 : (i * 16) + 12, :] = (0, 0, 200)
        rr.log("image", rr.Image(img))
        rr.log("rect/0", rr.Boxes2D(mins=[16, 16], sizes=[64, 64], labels="Rect0", colors=(255, 0, 0)))
        rr.log("rect/1", rr.Boxes2D(mins=[48, 48], sizes=[64, 64], labels="Rect1", colors=(0, 255, 0)))

    if not args.skip_blueprint:
        if args.auto_space_views:
            rr_exp.set_auto_space_views(True)

        rr_exp.set_panels(all_expanded=False)

        rr_exp.add_space_view(
            name="overlaid", space_view_class="2D", origin="/", entity_paths=["image", "rect/0", "rect/1"]
        )


if __name__ == "__main__":
    main()
