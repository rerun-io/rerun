"""Main conversion logic for RRD to LeRobot dataset conversion."""

from __future__ import annotations

import os
import shutil
import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa
from lerobot.datasets.compute_stats import compute_episode_stats
from lerobot.datasets.utils import update_chunk_file_indices
from tqdm import tqdm

from .types import FeatureSpec, LeRobotConversionConfig, RemuxData, RemuxInfo, VideoSpec
from .utils import normalize_times, to_float32_vector, unwrap_singleton
from .video_processing import can_remux_video, decode_video_frame, remux_video_stream

if TYPE_CHECKING:
    import datafusion as df
    from lerobot.datasets.lerobot_dataset import LeRobotDataset


def convert_dataframe_to_episode(
    df: df.DataFrame,
    config: LeRobotConversionConfig,
    *,
    video_data_cache: dict[str, tuple[list[bytes], np.ndarray]],
    lerobot_dataset: LeRobotDataset,
    segment_id: str,
    features: dict[str, FeatureSpec],
) -> tuple[bool, RemuxData | None, bool]:
    """
    Convert a DataFusion dataframe to a LeRobot episode.

    Args:
        df: DataFusion dataframe containing the segment data (already filtered and aligned)
        config: Conversion configuration
        video_data_cache: Pre-loaded video data per spec key (samples, times_ns)
        lerobot_dataset: LeRobot dataset to add frames to
        segment_id: ID of the segment being processed (for logging)
        features: Feature specifications from inference

    Returns:
        Tuple of (success, remux_data, direct_saved) where:
        - success: True if segment was processed successfully, False if skipped
        - remux_data: Dict with remuxing info if possible, None otherwise
        - direct_saved: True if the episode was saved without decoding video frames

    """
    action_dim = features["action"].shape[0] if "action" in features else None
    state_dim = features["observation.state"].shape[0] if "observation.state" in features else None

    if action_dim is None:
        raise ValueError("Action feature specification is missing.")

    if state_dim is None:
        raise ValueError("State feature specification is missing.")

    # Check if video remuxing is possible (when use_videos=True and FPS matches)
    remux_data: RemuxData | None = None
    if config.use_videos and config.videos:
        remux_info: dict[str, RemuxInfo] = {}
        for spec in config.videos:
            samples, times_ns = video_data_cache[spec.key]
            can_remux, source_fps = can_remux_video(times_ns, config.fps)
            if can_remux:
                remux_info[spec.key] = RemuxInfo(samples=samples, times_ns=times_ns, source_fps=source_fps)
            else:
                raise ValueError(
                    f"Video cannot be remuxed yet: spec={spec.key} source_fps={source_fps:.2f} target_fps={config.fps}"
                )

        remux_data = RemuxData(specs=config.videos, remux_info=remux_info, fps=config.fps)

    table = pa.table(df)
    if table.num_rows == 0:
        return False, None, False

    data_columns = {name: table[name].to_pylist() for name in table.column_names}
    num_rows = table.num_rows

    if config.use_videos and remux_data:
        _save_episode_without_video_decode(
            lerobot_dataset=lerobot_dataset,
            data_columns=data_columns,
            num_rows=num_rows,
            config=config,
            action_dim=action_dim,
            state_dim=state_dim,
            remux_data=remux_data,
        )
        return True, None, True

    # Decode video frames for the entire segment if needed
    video_frames = _decode_video_frames_for_batch(
        table,
        index_column=config.index_column,
        videos=config.videos,
        video_data_cache=video_data_cache,
    )

    for row_idx in tqdm(range(num_rows), desc=f"Frames ({segment_id})", leave=False):
        frame = _build_frame(
            row_idx=row_idx,
            data_columns=data_columns,
            config=config,
            action_dim=action_dim,
            state_dim=state_dim,
            video_frames=video_frames,
            num_rows=num_rows,
        )
        lerobot_dataset.add_frame(frame)

    return True, remux_data, False


def _get_video_path_for_episode(
    lerobot_dataset: LeRobotDataset,
    episode_index: int,
    video_key: str,
) -> Path:
    """
    Construct the video file path for an episode without loading metadata from disk.

    This mimics LeRobot's logic but works immediately after save_episode()
    before metadata is flushed.

    Args:
        lerobot_dataset: LeRobot dataset instance
        episode_index: Index of the episode
        video_key: Video key (e.g., "observation.images.camera")

    Returns:
        Path to the video file

    """

    # Get chunk and file indices from latest_episode (just saved)
    # latest_episode is set by save_episode() before it returns
    if lerobot_dataset.meta.latest_episode is None or episode_index == 0:
        # First episode or no latest episode
        chunk_idx, file_idx = 0, 0
        if lerobot_dataset.meta.episodes is not None and len(lerobot_dataset.meta.episodes) > 0:
            # Resuming - get from last episode
            old_chunk_idx = lerobot_dataset.meta.episodes[-1].get(f"videos/{video_key}/chunk_index", 0)
            old_file_idx = lerobot_dataset.meta.episodes[-1].get(f"videos/{video_key}/file_index", 0)
            chunk_idx, file_idx = update_chunk_file_indices(
                old_chunk_idx, old_file_idx, lerobot_dataset.meta.chunks_size
            )
    else:
        # Get from latest_episode which was just set by save_episode()
        latest_ep = lerobot_dataset.meta.latest_episode
        chunk_idx = latest_ep[f"videos/{video_key}/chunk_index"][0]
        file_idx = latest_ep[f"videos/{video_key}/file_index"][0]

    if lerobot_dataset.meta.video_path is None:
        raise ValueError("LeRobot dataset meta.video_path is not set.")

    # Construct path using the same format as LeRobot
    video_path = lerobot_dataset.meta.video_path.format(video_key=video_key, chunk_index=chunk_idx, file_index=file_idx)

    return lerobot_dataset.root / video_path


def apply_remuxed_videos(
    lerobot_dataset: LeRobotDataset,
    episode_index: int,
    remux_data: RemuxData,
) -> None:
    """
    Replace encoded videos with remuxed versions (100-1000x faster).

    After LeRobot's save_episode() encodes videos from PNGs, this function
    replaces them with directly remuxed videos from the original compressed
    packets. This skips the entire decode→PNG→re-encode cycle.

    Args:
        lerobot_dataset: LeRobot dataset instance
        episode_index: Index of the episode that was just saved
        remux_data: Dictionary with remuxing information

    """

    for spec in remux_data.specs:
        if spec.key not in remux_data.remux_info:
            continue

        info = remux_data.remux_info[spec.key]
        video_key = f"observation.images.{spec.key}"

        # Construct the video file path ourselves (can't use get_video_file_path yet)
        video_path = _get_video_path_for_episode(lerobot_dataset, episode_index, video_key)

        # Check if the video file was created by LeRobot
        if not video_path.exists():
            print(f"    WARNING: Video not found at {video_path}, skipping remux for '{spec.key}'")
            continue

        with tempfile.NamedTemporaryFile(suffix=".mp4", delete=False) as tmp_file:
            tmp_path = tmp_file.name

        try:
            remux_video_stream(
                samples=info.samples,
                times_ns=info.times_ns,
                output_path=tmp_path,
                video_format=spec.video_format,
                target_fps=remux_data.fps,
            )

            # Replace the encoded video with the remuxed one
            shutil.move(tmp_path, video_path)
            print(f"    ✓ Replaced encoded video with remuxed version: {video_path}")

        except Exception as e:
            print(f"    ✗ Failed to remux '{spec.key}': {e}")
            import traceback

            traceback.print_exc()
            # Keep the encoded version if remux fails
            if Path(tmp_path).exists():
                Path(tmp_path).unlink()


def _save_episode_without_video_decode(
    lerobot_dataset: LeRobotDataset,
    *,
    data_columns: dict[str, list[object]],
    num_rows: int,
    config: LeRobotConversionConfig,
    action_dim: int,
    state_dim: int,
    remux_data: RemuxData,
) -> None:
    """Save an episode without decoding video frames by remuxing source packets directly."""

    @contextmanager
    def _suppress_ffmpeg_output() -> None:
        with open(os.devnull, "w") as devnull:
            old_stdout_fd = os.dup(1)
            old_stderr_fd = os.dup(2)
            try:
                os.dup2(devnull.fileno(), 1)
                os.dup2(devnull.fileno(), 2)
                yield
            finally:
                os.dup2(old_stdout_fd, 1)
                os.dup2(old_stderr_fd, 2)
                os.close(old_stdout_fd)
                os.close(old_stderr_fd)

    episode_index = lerobot_dataset.meta.total_episodes
    task_values = data_columns.get(config.task, [None] * num_rows) if config.task else [None] * num_rows

    tasks: list[str] = []
    for task_value in task_values:
        task_value = unwrap_singleton(task_value)
        if task_value is None:
            task = config.task_default
        elif isinstance(task_value, (bytes, bytearray, memoryview)):
            task = bytes(task_value).decode("utf-8")
        else:
            task = str(task_value)
        tasks.append(task)

    episode_tasks = list(dict.fromkeys(tasks))
    lerobot_dataset.meta.save_episode_tasks(episode_tasks)
    task_index = np.array([lerobot_dataset.meta.get_task_index(task) for task in tasks], dtype=np.int64)

    start_index = lerobot_dataset.meta.total_frames
    times_ns = normalize_times(data_columns[config.index_column])
    times_s = (times_ns - times_ns[0]) / 1_000_000_000.0
    episode_buffer: dict[str, np.ndarray] = {
        "timestamp": times_s.astype(np.float32),
        "frame_index": np.arange(num_rows, dtype=np.int64),
        "episode_index": np.full((num_rows,), episode_index, dtype=np.int64),
        "index": np.arange(start_index, start_index + num_rows, dtype=np.int64),
        "task_index": task_index,
    }

    if action_dim is not None:
        action_values = [to_float32_vector(value, action_dim, "action") for value in data_columns[config.action]]
        episode_buffer["action"] = np.stack(action_values)

    if state_dim is not None:
        state_values = [to_float32_vector(value, state_dim, "state") for value in data_columns[config.state]]
        episode_buffer["observation.state"] = np.stack(state_values)

    ep_stats = compute_episode_stats(
        episode_buffer,
        {key: lerobot_dataset.features[key] for key in episode_buffer},
    )

    episode_metadata = lerobot_dataset._save_episode_data(episode_buffer)

    video_metadata: dict[str, object] = {}
    specs = remux_data.specs
    remux_info = remux_data.remux_info
    fps = remux_data.fps

    for spec in specs:
        info = remux_info.get(spec.key)
        if info is None:
            continue

        video_key = f"observation.images.{spec.key}"
        temp_dir = Path(tempfile.mkdtemp(dir=lerobot_dataset.root))
        temp_path = temp_dir / f"{video_key.replace('.', '_')}_{episode_index:03d}.mp4"

        with _suppress_ffmpeg_output():
            remux_video_stream(
                samples=info.samples,
                times_ns=info.times_ns,
                output_path=str(temp_path),
                video_format=spec.video_format,
                target_fps=fps,
            )

            video_metadata.update(
                lerobot_dataset._save_episode_video(
                    video_key,
                    episode_index,
                    temp_path=temp_path,
                )
            )

    video_metadata.pop("episode_index", None)
    episode_metadata.update(video_metadata)

    lerobot_dataset.meta.save_episode(
        episode_index,
        num_rows,
        episode_tasks,
        ep_stats,
        episode_metadata,
    )
    lerobot_dataset.episode_buffer = lerobot_dataset.create_episode_buffer()


def _decode_video_frames_for_batch(
    table: pa.Table,
    *,
    index_column: str,
    videos: list[VideoSpec],
    video_data_cache: dict[str, tuple[list[bytes], np.ndarray]],
) -> dict[str, list[np.ndarray]]:
    """
    Decode video frames for a batch of rows.

    Args:
        table: PyArrow table containing the batch
        index_column: Timeline column name
        videos: Video stream specifications
        video_data_cache: Cached video data per spec key
    Returns:
        Dictionary mapping spec key to list of decoded frames

    """
    from .utils import normalize_times

    video_frames: dict[str, list[np.ndarray]] = {}
    if video_data_cache:
        row_times_ns = normalize_times(table[index_column].to_pylist())
        for spec in videos:
            samples, times_ns = video_data_cache[spec.key]
            frames = []
            for time_ns in row_times_ns:
                frames.append(
                    decode_video_frame(
                        samples=samples, times_ns=times_ns, target_time_ns=int(time_ns), video_format=spec.video_format
                    )
                )
            video_frames[spec.key] = frames
    return video_frames


def _build_frame(
    *,
    row_idx: int,
    data_columns: dict[str, list[object]],
    config: LeRobotConversionConfig,
    action_dim: int,
    state_dim: int,
    video_frames: dict[str, list[np.ndarray]],
    num_rows: int,
) -> dict[str, object]:
    """
    Build a single frame dictionary for LeRobot dataset.

    Args:
        row_idx: Row index in the batch
        data_columns: Dictionary of column data
        config: Conversion configuration
        task_column: Fully qualified task column (or None)
        action_dim: Action dimension (if present)
        state_dim: State dimension (if present)
        task_default: Default task value
        image_specs: Image stream specifications
        video_frames: Decoded video frames per spec
        num_rows: Total number of rows in batch

    Returns:
        Frame dictionary ready for LeRobot dataset

    """
    frame: dict[str, object] = {}

    frame["action"] = to_float32_vector(
        data_columns[config.action][row_idx],
        action_dim,
        "action",
    )

    frame["observation.state"] = to_float32_vector(
        data_columns[config.state][row_idx],
        state_dim,
        "state",
    )

    # Add task
    task_value = data_columns.get(config.task, [None] * num_rows)[row_idx] if config.task else None
    task_value = unwrap_singleton(task_value)
    if task_value is None:
        task = config.task_default
    elif isinstance(task_value, (bytes, bytearray, memoryview)):
        task = bytes(task_value).decode("utf-8")
    else:
        task = str(task_value)
    frame["task"] = task

    # Add video frames
    for spec in config.videos:
        image = video_frames[spec.key][row_idx]
        frame[f"observation.images.{spec.key}"] = image

    return frame
