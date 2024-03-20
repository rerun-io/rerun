"""Disconnect two spaces."""
import rerun as rr

rr.init("rerun_example_disconnected_space", spawn=True)

# These two points can be projected into the same space..
rr.log("world/room1/point", rr.Points3D([[0, 0, 0]]))
rr.log("world/room2/point", rr.Points3D([[1, 1, 1]]))

# ..but this one lives in a completely separate space!
rr.log("world/wormhole", rr.DisconnectedSpace())
rr.log("world/wormhole/point", rr.Points3D([[2, 2, 2]]))
