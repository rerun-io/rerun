"""Log a pinhole and a random image."""
import numpy as np
import rerun as rr

rr.init("pinhole", spawn=True)
rng = np.random.default_rng(12345)

image = rng.uniform(0, 255, size=[3, 3, 3])
intrinsics = np.array([[3, 0, 1.5], [0, 3, 1.5], [0, 0, 1]])

rr.log_pinhole("world/image", child_from_parent=intrinsics, width=3, height=3)
rr.log_image("world/image", image=image)
