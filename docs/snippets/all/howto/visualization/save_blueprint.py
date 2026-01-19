"""Craft a blueprint with the python API and save it to a file for future use."""

import sys

import rerun.blueprint as rrb

path_to_rbl = sys.argv[1]

rrb.Blueprint(
    rrb.DataframeView(
        origin="/",
        query=rrb.archetypes.DataframeQuery(
            timeline="log_time",
            apply_latest_at=True,
        ),
    ),
).save("rerun_example_dataframe_view_query", path_to_rbl)
