"""Create and log a tensor."""

import numpy as np

import rerun as rr

tensor = np.random.default_rng(0).integers(
    0, 256, (8, 6, 3, 5), dtype=np.uint8
)  # 4-dimensional tensor

rr.init("rerun_example_tensor", spawn=True)

# Log the tensor, assigning names to each dimension
rr.log(
    "tensor",
    rr.Tensor(tensor, dim_names=("width", "height", "channel", "batch")),
)
