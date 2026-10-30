"""Use a blueprint to show a RawDataView."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_raw_data", spawn=True)

for i in range(10):
    rr.set_time("step", sequence=i)
    rr.log(
        "robot/arm",
        # Builtin types work just fine alongside custom ones.
        rr.Points3D([[i * 0.1, 0.0, 1.0]], radii=0.05, labels=[f"step {i}"]),
        rr.DynamicArchetype(
            archetype="ArmTelemetry",
            components={
                "temperature_celsius": 35.0 + i * 0.5,
                "motor_currents": [
                    0.2 + i * 0.01,
                    0.3 + i * 0.02,
                    0.1 + i * 0.01,
                ],
                "status": "moving" if i < 9 else "idle",
            },
        ),
    )

# Select built-in and custom components to display together.
blueprint = rrb.Blueprint(
    rrb.RawDataView(
        origin="/robot/arm",
        contents="$origin",
        name="Arm data",
        rows=rrb.RawDataRows(
            # Select specific rows to display. If omitted, all show up.
            data_rows=[
                "Points3D:positions",
                "Points3D:labels",
                "ArmTelemetry:motor_currents",
                "ArmTelemetry:status",
            ],
        ),
        format_options=rrb.RawDataViewFormat(
            show_time_navigation=True,
            style="Compact",
        ),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
