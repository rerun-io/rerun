"""Override a visualizer."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_component_override", spawn=True)

# Data logged to the data store.
rr.log("boxes/1", rr.Boxes2D(centers=[0, 0], sizes=[1, 1], colors=[255, 0, 0]))
rr.log("boxes/2", rr.Boxes2D(centers=[2, 0], sizes=[1, 1]))

rr.send_blueprint(
    rrb.Spatial2DView(
        overrides={
            "boxes/1": [
                # Specify multiple visualizers
                rrb.VisualizerOverrides([
                    rrb.visualizers.Boxes2D,
                    rrb.visualizers.Points2D,
                ]),
                rr.components.Color([0, 255, 0]),
            ]
        },
        defaults=[rr.components.Color([0, 0, 255])],
    ),
)
