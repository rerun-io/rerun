"""Log a batch of 3D arrows."""

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_clear_simple", spawn=True)

vectors = [(1.0, 0.0, 0.0), (0.0, -1.0, 0.0), (-1.0, 0.0, 0.0), (0.0, 1.0, 0.0)]
origins = [(-0.5, 0.5, 0.0), (0.5, 0.5, 0.0), (0.5, -0.5, 0.0), (-0.5, -0.5, 0.0)]
colors = [(200, 0, 0), (0, 200, 0), (0, 0, 200), (200, 0, 200)]

# Log a handful of arrows.
for i, (vector, origin, color) in enumerate(zip(vectors, origins, colors)):
    rr2.log(f"arrows/{i}", rr2.Arrows3D(vectors=vector, origins=origin, colors=color))

# Now clear all of them at once.
# TODO(#3268): `rr2.Clear.recursive()`
rr2.log("arrows", rr2.Clear(True))
