"""Load a LeRobot dataset using the Python SDK."""

import sys

import rerun as rr

path_to_lerobot_dataset = sys.argv[1]

# Initialize the SDK and give our recording a unique name
rr.init("rerun_example_load_lerobot", spawn=True)

# Load the LeRobot dataset (a directory of metadata, parquet, and video files)
rr.log_file_from_path(path_to_lerobot_dataset)
recording = rr.get_data_recording()
assert recording is not None
recording.flush()
