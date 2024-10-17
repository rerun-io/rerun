"""Query and display the first 10 rows of a recording in a dataframe view."""

import sys

import rerun as rr
import rerun.blueprint as rrb

path_to_rrd = sys.argv[1]

rr.init("rerun_example_dataframe", spawn=True)
rr.log_file_from_path(path_to_rrd)

blueprint = rrb.Blueprint(
    rrb.DataframeView(
        origin="/",
        query=rrb.archetypes.DataframeQuery(
            timeline="log_time",
        ),
    ),
)
rr.send_blueprint(blueprint)

# TODO: blocked by https://github.com/rerun-io/rerun/issues/7819
