"""Configure playback in the time panel from a blueprint."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_playback_blueprint", spawn=True)

for frame in range(60):
    rr.set_time("frame", sequence=frame)
    rr.log("world/point", rr.Points3D([[frame / 60.0, 0.0, 0.0]], radii=0.05))

# region: blueprint
rr.send_blueprint(
    rrb.Blueprint(
        rrb.Spatial3DView(origin="/world"),
        rrb.TimePanel(
            timeline="frame",
            playback_speed=2.0,
            fps=30.0,
            loop_mode="all",
        ),
    )
)
# endregion: blueprint
