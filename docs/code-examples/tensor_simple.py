"""Create and log a tensor."""

import numpy as np
import rerun as rr

# Create a 4-dimensional tensor
tensor = np.random.uniform(0.0, 1.0, (8, 6, 3, 5))

rr.init("tensors", spawn=True)

# Log the tensor, assigning names to each dimension
rr.log_tensor("tensor", tensor, names=("width", "height", "channel", "batch"))
