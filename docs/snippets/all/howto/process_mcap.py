"""Read and convert an MCAP file with the Python Chunk Processing API."""

from __future__ import annotations

import os
import sys
from pathlib import Path

mcap_path = Path(sys.argv[1])
output_path = Path(os.environ.get("_RERUN_TEST_FORCE_SAVE", "output.rrd"))

# region: example
from rerun.chunk import McapReader

reader = McapReader(mcap_path)
print(reader.info().message_count)

# Stream chunks, optionally filter or transform them, and write to RRD.
reader.stream().write_rrd(
    output_path,
    application_id="rerun_example_process_mcap",
    recording_id="example",
)
# endregion: example
