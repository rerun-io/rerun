"""
Visualize an arbitrary component as state by remapping `StateChange.state`.

⚠️TODO(#12600): The API for component mappings is still evolving, so this
example may change in the future.
"""

from __future__ import annotations

import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.datatypes import (
    ComponentSourceKind,
    VisualizerComponentMapping,
)

rr.init("rerun_example_state_remapping", spawn=True)

# region: custom_data
# Log a robot mode as a plain string component — note that this is *not* a
# `StateChange`, just an arbitrary string logged via `AnyValues`. It shows up on
# the entity as the component `AnyValues:mode`.
modes = ["booting", "idle", "driving", "idle", "charging"]
for step, mode in enumerate(modes):
    rr.set_time("step", sequence=step)
    rr.log("robot", rr.AnyValues(mode=mode))
# endregion: custom_data

# region: blueprint
# Remap the state-timeline visualizer's `StateChange:state` input to read from
# the custom `AnyValues:mode` component instead of an actual `StateChange`.
# Any string, boolean, or numeric component can be visualized this way.
blueprint = rrb.Blueprint(
    rrb.StateTimelineView(
        origin="/",
        name="Robot mode",
        overrides={
            "robot": [
                rr.StateChange.from_fields().visualizer(
                    mappings=[
                        VisualizerComponentMapping(
                            target="StateChange:state",
                            source_kind=ComponentSourceKind.SourceComponent,
                            source_component="AnyValues:mode",
                        ),
                    ],
                ),
            ],
        },
    ),
)
rr.send_blueprint(blueprint)
# endregion: blueprint
