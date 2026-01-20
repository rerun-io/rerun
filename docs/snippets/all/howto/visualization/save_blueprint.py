"""Craft a blueprint with the python API and save it to a file for future use."""

import sys

import rerun.blueprint as rrb

path_to_rbl = sys.argv[1]

blueprint = rrb.Blueprint(
    rrb.TimeSeriesView(name="AAPL", origin="/stocks/AAPL"),
)

# Save to a file
blueprint.save("rerun_example_blueprint_stocks", path_to_rbl)
