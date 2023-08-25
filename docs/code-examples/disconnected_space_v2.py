"""Log some very simple points."""
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun-example-disconnect_space", spawn=True)

# These two points can be projected into the same space..
rr2.log("world/room1/point", rr2.Points3D([[0, 0, 0]]))
rr2.log("world/room2/point", rr2.Points3D([[1, 1, 1]]))

# ..but this one lives in a completely separate space!
rr2.log("world/wormhole", rr2.DisconnectedSpace(True))
rr2.log("world/wormhole/point", rr2.Points3D([[2, 2, 2]]))
