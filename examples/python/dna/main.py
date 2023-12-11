#!/usr/bin/env python3
"""
The example from our Getting Started page.

`examples/python/dna/main.py`
"""
from __future__ import annotations

import argparse
from math import tau

import numpy as np
import rerun as rr  # pip install rerun-sdk
from rerun.utilities import bounce_lerp, build_color_spiral

DESCRIPTION = """
# DNA
This is a minimal example that logs synthetic 3D data in the shape of a double helix. The underlying data is generated
using numpy and visualized using Rerun.

## How it was made
The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/dna/main.py).

### Colored 3D points
The colored 3D points were added to the scene by logging the
[rr.Points3D archetype](https://www.rerun.io/docs/reference/types/archetypes/points3d) to the
[helix/structure/left](recording://helix/structure/left) and [helix/structure/right](recording://helix/structure/right)
entities.

### 3D line strips
The 3D line strips connecting the 3D point pairs are logged as an
[rr.LineStrips3D archetype](https://www.rerun.io/docs/reference/types/archetypes/line_strips3d) to the
[helix/structure/scaffolding entity](recording://helix/structure/scaffolding).

### Rotation
The whole structure is rotated over time by logging a
[rr.Transform3D archetype](https://www.rerun.io/docs/reference/types/archetypes/transform3d) to the
[helix/structure entity](recording://helix/structure:Transform3D) that changes over time. This transform determines the rotation of
the [structure entity](recording://helix/structure) relative to the [helix](recording://helix) entity. Since all other
entities are children of [helix/structure](recording://helix/structure) they will also rotate based on this transform.

You can visualize this rotation by selecting the two entities on the left-hand side and activating `Show transform` in
the Blueprint settings on the right-hand side. You will see one static frame (i.e., the frame of
[helix](recording://helix)) and the rotating frame (i.e., the frame of [structure](recording://helix/structure)).
""".strip()


def log_data() -> None:
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), timeless=True)

    rr.set_time_seconds("stable_time", 0)

    NUM_POINTS = 100

    # points and colors are both np.array((NUM_POINTS, 3))
    points1, colors1 = build_color_spiral(NUM_POINTS)
    points2, colors2 = build_color_spiral(NUM_POINTS, angular_offset=tau * 0.5)
    rr.log("helix/structure/left", rr.Points3D(points1, colors=colors1, radii=0.08))
    rr.log("helix/structure/right", rr.Points3D(points2, colors=colors2, radii=0.08))

    rr.log("helix/structure/scaffolding", rr.LineStrips3D(np.stack((points1, points2), axis=1), colors=[128, 128, 128]))

    time_offsets = np.random.rand(NUM_POINTS)
    for i in range(400):
        time = i * 0.01
        rr.set_time_seconds("stable_time", time)

        times = np.repeat(time, NUM_POINTS) + time_offsets
        beads = [bounce_lerp(points1[n], points2[n], times[n]) for n in range(NUM_POINTS)]
        colors = [[int(bounce_lerp(80, 230, times[n] * 2))] for n in range(NUM_POINTS)]
        rr.log(
            "helix/structure/scaffolding/beads", rr.Points3D(beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1))
        )

        rr.log(
            "helix/structure",
            rr.Transform3D(rotation=rr.RotationAxisAngle(axis=[0, 0, 1], radians=time / 4.0 * tau)),
        )


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_dna_abacus")
    log_data()
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
