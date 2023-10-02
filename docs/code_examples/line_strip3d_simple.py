"""Log a simple line strip."""
import rerun as rr

rr.init("rerun_example_line_strip3d", spawn=True)

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

rr.log("strip", rr.LineStrips3D([points]))
