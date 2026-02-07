"""Load an MCAP file using the Python SDK."""

import sys

import rerun as rr

path_to_mcap = sys.argv[1]

# Initialize the SDK and give our recording a unique name
rr.init("rerun_example_load_mcap", spawn=True)

# Load the MCAP file
rr.log_file_from_path(path_to_mcap)
