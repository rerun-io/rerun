"""Walk through a basic chunk-processing pipeline: read, filter, write."""

from __future__ import annotations

from pathlib import Path

mcap_path = (
    Path(__file__).resolve().parents[4]
    / "tests"
    / "assets"
    / "mcap"
    / "trossen_transfer_cube.mcap"
)
output_path = Path("chunk_processing_intro.rrd")

# region: read
from rerun.experimental import McapReader

stream = McapReader(mcap_path).stream()
# endregion: read

# region: filter
stream = stream.filter(content="/robot_left/**")
# endregion: filter

# region: terminal
stream.write_rrd(
    output_path,
    application_id="rerun_example_chunk_processing_intro",
    recording_id="run1",
)
# endregion: terminal
