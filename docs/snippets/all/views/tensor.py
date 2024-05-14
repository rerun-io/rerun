"""Use a blueprint to show a tensor view."""

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_tensor", spawn=True)

tensor_one = np.random.randint(0, 256, (8, 6, 3, 5), dtype=np.uint8)
rr.log("tensors/one", rr.Tensor(tensor_one, dim_names=("width", "height", "channel", "batch")))
tensor_two = np.random.random_sample((10, 20, 30))
rr.log("tensors/two", rr.Tensor(tensor_two))

# Create a tensor view that displays both tensors (you can switch between them inside the view).
blueprint = rrb.Blueprint(rrb.TensorView(origin="/tensors", name="Tensors"), collapse_panels=True)

rr.send_blueprint(blueprint)
