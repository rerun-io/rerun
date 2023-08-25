"""Log a simple line strip."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun-example-line_strip3d", spawn=True)

points = [
    [0, 0, 0],
    [0, 0, 1],
    [1, 0, 0],
    [1, 0, 1],
    [1, 1, 0],
    [1, 1, 1],
    [0, 1, 0],
    [0, 1, 1],
]

rr2.log("strip", rr2.LineStrips3D([points]))
