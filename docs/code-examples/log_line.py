import rerun as rr

# 0.8
rr.log_point("my_point", [1.0, 2.0, 3.0])

# 0.9
rr.log("my_point", rr.Points3D([1.0, 2.0, 3.0]))
