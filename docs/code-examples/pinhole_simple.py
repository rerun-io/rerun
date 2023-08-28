"""Log a pinhole and a random image."""
import numpy as np
import rerun as rr

rr.init("rerun_example_pinhole", spawn=True)
rng = np.random.default_rng(12345)

image = rng.uniform(0, 255, size=[3, 3, 3])
rr.log_pinhole("world/image", focal_length_px=3, width=3, height=3)
rr.log_image("world/image", image=image)
