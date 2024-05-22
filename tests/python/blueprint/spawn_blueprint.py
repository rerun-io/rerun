from __future__ import annotations

import rerun.blueprint as rrb

blueprint = rrb.Blueprint(
    rrb.Spatial3DView(origin="/test1"),
    rrb.TimePanel(state="collapsed"),
    rrb.SelectionPanel(state="collapsed"),
    rrb.BlueprintPanel(state="collapsed"),
)

blueprint.spawn("rerun_example_blueprint_test")
