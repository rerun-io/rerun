"""Create and log a tensor."""

import rerun as rr
from numpy.random import default_rng

rng = default_rng(12345)

# Create a 4-dimensional tensor
tensor = rng.uniform(0.0, 1.0, (8, 6, 3, 5))

rr.init("rerun-example-tensors", spawn=True)

# Log the tensor, assigning names to each dimension
rr.log_tensor("tensor", tensor, names=("width", "height", "channel", "batch"))
