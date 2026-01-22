# region: setup
from __future__ import annotations

import os
import shutil
from pathlib import Path

import pyarrow as pa
import rerun as rr
from datafusion import col, functions as F
from lerobot.datasets.lerobot_dataset import LeRobotDataset  # type: ignore[import-untyped]
from rerun_export.lerobot.converter import convert_dataframe_to_episode
from rerun_export.lerobot.feature_inference import infer_features
from rerun_export.lerobot.types import LeRobotConversionConfig, VideoSpec
from rerun_export.utils import make_time_grid

# Start a server with RRD recordings
# In practice, you would point this to your directory of RRD files
# TODO(gijsd): replace with a real dataset path
sample_5_path = "/Users/gijsd/rerun-io/rrds-demo/"

server = rr.server.Server(datasets={"robot_dataset": sample_5_path})
client = server.client()
dataset = client.get_dataset(name="robot_dataset")
# endregion: setup

# region: filter_data
# Select a single recording (episode) to export
single_recording = dataset.segment_ids()[0]

# Filter the dataset to include only the data we need for training:
# - Action commands sent to the robot
# - Observed joint positions (robot state)
# - Camera feeds from multiple viewpoints
# - Task descriptions (e.g., language instructions)
training_data = (
    dataset.filter_segments(single_recording)
    .filter_contents([
        "/robot_right/**",
        "/robot_left/**",
        "/external/**",
        "/task_description",
    ])
    .reader(index="real_time")
)
# endregion: filter_data
# Define how to extract task instructions from the recording
# This could be from a TextDocument, static metadata, etc.
# For this example, we assume a static instruction
instructions = "/task_description:foxglove.KeyValuePair:message"

# Specify video streams to include in the dataset
# Each stream needs a key (camera identifier) and path to the VideoStream component
videos = [
    VideoSpec(key="cam_high", path="/external/cam_high/video_compressed", video_format="h264"),
    VideoSpec(key="cam_low", path="/external/cam_low/video_compressed", video_format="h264"),
    VideoSpec(key="wrist", path="/robot_right/wrist_camera/video_compressed", video_format="h264"),
]

# Configure the conversion parameters
# This maps Rerun's flexible data model to LeRobot's standardized format
config = LeRobotConversionConfig(
    fps=50,  # Target framerate for the dataset
    index_column="message_publish_time",  # Timeline to use for alignment
    action="/robot_left/joint_states:schemas.proto.JointState:joint_positions",  # Fully qualified action column
    state="/robot_right/joint_states:schemas.proto.JointState:joint_positions",  # Fully qualified state column
    task=instructions,  # Task description column
    videos=videos,  # Video streams to include
    dataset=dataset,
    segment_id=single_recording,
)
# endregion: configure_export

# region: infer_features
# Infer the LeRobot feature schema from the data
# This automatically detects data types, shapes, and creates the appropriate
# LeRobot feature definitions
features = infer_features(
    table=training_data.to_arrow_table(),
    config=config,
)
# endregion: infer_features

# region: create_dataset
# Create the LeRobot dataset structure on disk
dataset_root = "./lerobot_dataset"
if os.path.exists(dataset_root):
    print("Removing old dataset export")
    shutil.rmtree(dataset_root)

lerobot_dataset = LeRobotDataset.create(
    repo_id="rerun/lerobot_dataset",  # Dataset identifier
    fps=config.fps,
    features=features,  # Feature schema
    root=dataset_root,  # Output directory
    use_videos=config.use_videos,  # Store videos (vs. individual images)
    video_backend="h264x",  # Video encoding backend
)
print("Initialized LeRobot dataset in:", dataset_root)
# endregion: create_dataset

# region: export_episode
# Convert the recording to a LeRobot episode
# This aligns all time series to the target framerate, extracts video frames,
# and writes the episode data in LeRobot's Parquet format
print("Creating episode")

convert_dataframe_to_episode(
    df=training_data,
    config=config,
    lerobot_dataset=lerobot_dataset,
    segment_id=single_recording,
    features=features,
)

# Finalize the dataset (write metadata, close files, etc.)
lerobot_dataset.finalize()

print("Done!")
# endregion: export_episode

rr.init("lerobot_export_example", spawn=True)
rr.log_file_from_path(dataset_root)
