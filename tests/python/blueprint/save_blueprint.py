from __future__ import annotations

import rerun.blueprint as rrb

blueprint = rrb.Blueprint(
    rrb.Spatial3DView(origin="/test1"),
    rrb.TimePanel(expanded=False),
    rrb.SelectionPanel(expanded=False),
    rrb.BlueprintPanel(expanded=False),
)

blueprint.save("rerun_example_blueprint_test.rbl")
