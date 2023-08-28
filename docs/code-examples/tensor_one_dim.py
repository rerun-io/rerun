"""Create and log a one dimensional tensor."""

import rerun as rr
from numpy.random import default_rng

rng = default_rng(12345)

# Create a 1-dimensional tensor
tensor = rng.laplace(0.0, 1.0, 100)

rr.init("rerun_example_tensors", spawn=True)

# Log the tensor, assigning names to each dimension
rr.log_tensor("tensor", tensor)
