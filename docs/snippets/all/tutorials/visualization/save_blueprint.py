"""Craft an example blueprint with the python API and save it to a file for future use."""

import sys

import rerun.blueprint as rrb

path_to_rbl = sys.argv[1]

rrb.Blueprint(
    rrb.Horizontal(
        rrb.Grid(
            rrb.BarChartView(name="Bar Chart", origin="/bar_chart"),
            rrb.TimeSeriesView(
                name="Curves",
                origin="/curves",
            ),
        ),
        rrb.TextDocumentView(name="Description", origin="/description"),
        column_shares=[3, 1],
    ),
    rrb.SelectionPanel(state="collapsed"),
    rrb.TimePanel(state="collapsed"),
).save("your_blueprint_name", path_to_rbl)
