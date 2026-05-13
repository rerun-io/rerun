from rerun.experimental import McapReader

McapReader("input.mcap").stream().write_rrd(
    "run-1.rrd",
    application_id="rerun_example_getting_started",
    recording_id="run-1",
)
