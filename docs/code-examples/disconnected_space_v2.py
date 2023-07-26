"""Log some very simple points."""
import rerun as rr
import rerun.experimental as rr_exp

rr.init("disconnect_space", spawn=True)

# These two points can be projected into the same space..
rr_exp.log_any("world/room1/point", rr_exp.Points3D([[0, 0, 0]]))
rr_exp.log_any("world/room2/point", rr_exp.Points3D([[1, 1, 1]]))

# ..but this one lives in a completely separate space!
rr_exp.log_any("world/wormhole", rr_exp.DisconnectedSpace(True))
rr_exp.log_any("world/wormhole/point", rr_exp.Points3D([[2, 2, 2]]))
