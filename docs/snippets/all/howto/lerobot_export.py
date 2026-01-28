"""Demonstrate converting Rerun recording to LeRobot dataset."""

# ruff: noqa: E402
from __future__ import annotations

import atexit
import pathlib
import shutil
import tempfile

TMP_DIR = pathlib.Path(tempfile.mkdtemp())
atexit.register(lambda: shutil.rmtree(TMP_DIR) if TMP_DIR.exists() else None)

# region: setup

from pathlib import Path

import rerun as rr
from lerobot.datasets.lerobot_dataset import LeRobotDataset  # type: ignore[import-untyped]
from rerun_export.lerobot.converter import convert_dataframe_to_episode
from rerun_export.lerobot.feature_inference import infer_features
from rerun_export.lerobot.types import LeRobotConversionConfig, VideoSpec

# Start a server with RRD recordings
# In practice, you would point this to your directory of RRD files
sample_5_path = Path(__file__).parents[4] / "tests" / "assets" / "rrd" / "sample_5"

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
        "/action/joint_positions",
        "/observation/joint_positions",
        "/camera/**",
        "/language_instruction",
    ])
    .reader(index="real_time")
)

# endregion: filter_data

# region: configure_export
# Define how to extract task instructions from the recording
# This could be from a TextDocument, static metadata, etc.
# For this example, we assume a static instruction
instructions = "/language_instruction:TextDocument:text"

# Specify video streams to include in the dataset
# Each stream needs a key (camera identifier) and entity path where the VideoStream is logged
videos = [
    VideoSpec(key="ext1", path="/camera/ext1", video_format="h264"),
    VideoSpec(key="ext2", path="/camera/ext2", video_format="h264"),
    VideoSpec(key="wrist", path="/camera/wrist", video_format="h264"),
]

# Configure the conversion parameters
# This maps Rerun's flexible data model to LeRobot's standardized format
config = LeRobotConversionConfig(
    fps=15,  # Target framerate for the dataset
    index_column="real_time",  # Timeline to use for alignment
    action="/action/joint_positions:Scalars:scalars",  # Fully qualified action column
    state="/observation/joint_positions:Scalars:scalars",  # Fully qualified state column
    task=instructions,  # Task description column
    videos=videos,  # Video streams to include
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
lerobot_dataset = LeRobotDataset.create(
    repo_id="rerun/droid_lerobot",  # Dataset identifier
    fps=config.fps,
    features=features,
    root=TMP_DIR / "lerobot_dataset",
    use_videos=config.use_videos,
)
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
# endregion: export_episode

# region: multi_episode_export
# Create a new LeRobot dataset for multiple episodes
lerobot_dataset = LeRobotDataset.create(
    repo_id="rerun/droid_lerobot_full",
    fps=config.fps,
    features=features,
    root=TMP_DIR / "lerobot_dataset_full",
    use_videos=config.use_videos,
)

# To export multiple recordings, repeat the filtering and conversion steps
for segment_id in dataset.segment_ids():
    print(f"Exporting segment: {segment_id}")

    segment_data = (
        dataset.filter_segments(segment_id)
        .filter_contents([
            "/action/joint_positions",
            "/observation/joint_positions",
            "/camera/**",
            "/language_instruction",
        ])
        .reader(index="real_time")
    )

    convert_dataframe_to_episode(
        df=segment_data,
        config=config,
        lerobot_dataset=lerobot_dataset,
        segment_id=segment_id,
        features=features,
    )

lerobot_dataset.finalize()
# endregion: multi_episode_export

# region: use_dataset

# Access episode data
print(f"Total episodes: {lerobot_dataset.num_episodes}")
print(f"Total frames: {lerobot_dataset.num_frames}")

# endregion: use_dataset
