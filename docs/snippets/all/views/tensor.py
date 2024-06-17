"""Use a blueprint to show a tensor view."""

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_tensor", spawn=True)

tensor = np.random.randint(0, 256, (8, 6, 3, 5), dtype=np.uint8)
rr.log("tensor", rr.Tensor(tensor, dim_names=("width", "height", "channel", "batch")))

blueprint = rrb.Blueprint(
    rrb.TensorView(
        origin="tensor",
        name="Tensor",
        # Set a scalar mapping with a custom colormap, gamma and magnification filter.
        scalar_mapping=rrb.TensorScalarMapping(colormap="turbo", gamma=1.5, mag_filter="linear"),
        # Change sizing mode to keep aspect ratio.
        view_fit="FillKeepAspectRatio",
    ),
    collapse_panels=True,
)
rr.send_blueprint(blueprint)
