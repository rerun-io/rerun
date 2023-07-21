"""Log some very simple points."""
import rerun as rr
import rerun.experimental as rr_exp

rr.init("points", spawn=True)

rr_exp.log_any("simple", rr_exp.Points3D([[0, 0, 0], [1, 1, 1]]))
