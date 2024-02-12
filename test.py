import rerun as rr
import numpy as np

rr.init("test", spawn=True)

rr.log("images/image1", rr.Image(np.random.rand(100, 100, 3)))
rr.log("images/image2", rr.Image(np.random.rand(100, 100, 3)))
rr.log("images/image3", rr.Image(np.random.rand(100, 100, 3)))
