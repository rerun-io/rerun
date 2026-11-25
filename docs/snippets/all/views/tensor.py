"""Use a blueprint to show a tensor view."""

import numpy as np

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_tensor", spawn=True)

tensor = np.random.randint(0, 256, (32, 240, 320, 3), dtype=np.uint8)
rr.log("tensor", rr.Tensor(tensor, dim_names=("batch", "x", "y", "channel")))

blueprint = rrb.Blueprint(
    rrb.TensorView(
        origin="tensor",
        name="Tensor",
        # Explicitly pick which dimensions to show.
        slice_selection=rrb.TensorSliceSelection(
            # Width and height select axes; indices select slices on the
            # remaining axes.
            width=1,
            height=rr.TensorDimensionSelection(dimension=2, invert=True),
            # Set which indices to show for the other dimensions.
            indices=[
                rr.TensorDimensionIndexSelection(dimension=0, index=4),
                rr.TensorDimensionIndexSelection(dimension=3, index=2),
            ],
            # Dimensions omitted from `slider` keep a fixed index.
            slider=[0],
        ),
        # Set a scalar mapping with a custom colormap, gamma and
        # magnification filter.
        scalar_mapping=rrb.TensorScalarMapping(
            colormap="turbo", gamma=1.5, mag_filter="linear"
        ),
        # Fill the view, ignoring aspect ratio.
        view_fit="fill",
    ),
    collapse_panels=True,
)
rr.send_blueprint(blueprint)
