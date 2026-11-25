"""Set the eye position and look target of a 3D view from code."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_spatial3d_eye_controls", spawn=True)

rr.log(
    "world/points",
    rr.Points3D([[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]], radii=0.05),
)

rr.send_blueprint(
    rrb.Spatial3DView(
        origin="/world",
        eye_controls=rrb.EyeControls3D(
            position=[4.0, -4.0, 3.0],
            look_target=[0.0, 0.0, 0.0],
            eye_up=[0.0, 0.0, 1.0],
        ),
    )
)
