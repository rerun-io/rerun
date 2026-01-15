"""Main conversion logic for RRD to LeRobot dataset conversion."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import rerun as rr
from datafusion import col, functions as F, DataFrame as DataFusionDataFrame
from tqdm import tqdm

from .feature_inference import infer_features
from .types import ColumnSpec, ConversionConfig, ImageSpec
from .utils import make_time_grid, to_float32_vector, unwrap_singleton
from .video_processing import can_remux_video, decode_video_frame, extract_video_samples, remux_video_stream

if TYPE_CHECKING:
    from pathlib import Path
    import numpy as np
    from collections.abc import Iterable
    from lerobot.datasets.lerobot_dataset import LeRobotDataset


def convert_dataframe_to_episode(
    df: DataFusionDataFrame,
    config: ConversionConfig,
    video_data_cache: dict[str, tuple[list[bytes], np.ndarray]],
    lerobot_dataset: LeRobotDataset,
    segment_id: str,
    features: dict[str, dict],
) -> tuple[bool, dict | None, bool]:
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
    action_dim = features["action"]["shape"][0] if "action" in features else None
    state_dim = features["observation.state"]["shape"][0] if "observation.state" in features else None

    # Check if video remuxing is possible (when use_videos=True and FPS matches)
    remux_data = None
    if config.use_videos and config.image_specs:
        all_can_remux = True
        remux_info = {}
        for spec in config.image_specs:
            samples, times_ns = video_data_cache[spec.key]
            can_remux, source_fps = can_remux_video(times_ns, config.fps)
            if can_remux:
                remux_info[spec.key] = {
                    "samples": samples,
                    "times_ns": times_ns,
                    "source_fps": source_fps,
                }
                print(f"  ✓ Video '{spec.key}' can be remuxed (source: {source_fps:.2f}fps, target: {config.fps}fps)")
            else:
                all_can_remux = False
                print(f"  ✗ Video '{spec.key}' requires re-encoding (source: {source_fps:.2f}fps, target: {config.fps}fps)")

        if all_can_remux:
            remux_data = {
                "specs": config.image_specs,
                "remux_info": remux_info,
                "video_format": config.video_format,
                "fps": config.fps,
            }
            print(f"  🚀 Fast path: will remux {len(remux_info)} video(s) directly (100-1000x faster!)")
        else:
            print("  ⚠️  Slow path: will decode and re-encode videos (FPS mismatch)")

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
            index_column=config.index_column,
            columns=config.columns,
            action_dim=action_dim,
            state_dim=state_dim,
            task_default=config.task_default,
            fps=config.fps,
            remux_data=remux_data,
        )
        return True, None, True

    # Decode video frames for the entire segment if needed
    video_frames = _decode_video_frames_for_batch(
        table=table,
        index_column=config.index_column,
        image_specs=config.image_specs,
        video_data_cache=video_data_cache,
        video_format=config.video_format,
    )

    for row_idx in tqdm(range(num_rows), desc=f"Frames ({segment_id})", leave=False):
        frame = _build_frame(
            row_idx=row_idx,
            data_columns=data_columns,
            columns=config.columns,
            action_dim=action_dim,
            state_dim=state_dim,
            task_default=config.task_default,
            image_specs=config.image_specs,
            video_frames=video_frames,
            num_rows=num_rows,
        )
        lerobot_dataset.add_frame(frame)

    return True, remux_data, False


def convert_rrd_dataset_to_lerobot(
    rrd_dir: Path,
    output_dir: Path,
    dataset_name: str,
    repo_id: str,
    fps: int,
    index_column: str,
    action_path: str | None,
    state_path: str | None,
    task_path: str | None,
    task_default: str,
    image_specs: list[ImageSpec],
    segments: Iterable[str] | None,
    max_segments: int | None,
    use_videos: bool,
    action_names: list[str] | None,
    state_names: list[str] | None,
    vcodec: str,
    video_format: str,
    action_column: str | None = None,
    state_column: str | None = None,
    task_column: str | None = None,
) -> None:
    """
    Convert a directory of RRD recordings to a LeRobot v3 dataset.

    Args:
        rrd_dir: Directory containing RRD recordings
        output_dir: Output directory for the LeRobot dataset
        dataset_name: Catalog dataset name
        repo_id: LeRobot repo ID
        fps: Target dataset FPS
        index_column: Timeline to align on (e.g., "real_time")
        action_path: Rerun entity path for actions
        state_path: Rerun entity path for state observations
        task_path: Rerun entity path for task text
        task_default: Fallback task label when missing
        image_specs: List of image stream specifications
        segments: Optional list of segment IDs to convert
        max_segments: Limit number of segments to convert
        use_videos: Store images as videos instead of individual frames
        action_names: Optional names for action dimensions
        state_names: Optional names for state dimensions
        vcodec: Video codec for LeRobot encoding
        video_format: Video stream codec format for decoding
        action_column: Override action column name
        state_column: Override state column name
        task_column: Override task column name

    Raises:
        ValueError: If inputs are invalid or conversion fails

    """
    from lerobot.datasets.lerobot_dataset import LeRobotDataset

    if not rrd_dir.is_dir():
        raise ValueError(f"RRD directory does not exist or is not a directory: {rrd_dir}")
    if output_dir.exists():
        raise ValueError(f"Output directory already exists: {output_dir}")
    if action_path is None and state_path is None and not image_specs:
        raise ValueError("At least one of --action-path, --state-path, or --image must be provided.")

    if action_column is None and action_path:
        action_column = action_path if ":" in action_path else f"{action_path}:Scalars:scalars"
    if state_column is None and state_path:
        state_column = state_path if ":" in state_path else f"{state_path}:Scalars:scalars"
    if task_column is None and task_path:
        task_column = task_path if ":" in task_path else f"{task_path}:TextDocument:text"
    columns = ColumnSpec(action=action_column, state=state_column, task=task_column)

    with rr.server.Server(datasets={dataset_name: rrd_dir}) as server:
        client = server.client()
        dataset = client.get_dataset(name=dataset_name)
        segment_ids = list(segments) if segments else dataset.segment_ids()
        if max_segments is not None:
            segment_ids = segment_ids[:max_segments]
        if not segment_ids:
            raise ValueError("No segments found in the dataset.")

        features = infer_features(
            dataset=dataset,
            segment_id=segment_ids[0],
            index_column=index_column,
            columns=columns,
            image_specs=image_specs,
            use_videos=use_videos,
            action_names=action_names,
            state_names=state_names,
            video_format=video_format,
        )
        lerobot_dataset = LeRobotDataset.create(
            repo_id=repo_id,
            fps=fps,
            features=features,
            root=output_dir,
            use_videos=use_videos,
            video_backend=vcodec,
        )

        # Process each segment (recording) separately
        for segment_id in tqdm(segment_ids, desc="Segments"):
            try:
                success, remux_data, direct_saved = _process_segment(
                    dataset=dataset,
                    segment_id=segment_id,
                    index_column=index_column,
                    columns=columns,
                    action_path=action_path,
                    state_path=state_path,
                    task_path=task_path,
                    task_default=task_default,
                    image_specs=image_specs,
                    fps=fps,
                    features=features,
                    lerobot_dataset=lerobot_dataset,
                    video_format=video_format,
                    use_videos=use_videos,
                )
                if success and not direct_saved:
                    episode_index = lerobot_dataset.episode_buffer["episode_index"]
                    lerobot_dataset.save_episode()

                    # If remuxing is possible, replace encoded videos with remuxed versions
                    if use_videos and remux_data:
                        _apply_remuxed_videos(lerobot_dataset, episode_index, remux_data)

            except Exception as e:
                print(f"Error processing segment {segment_id}: {e}")
                import traceback

                traceback.print_exc()
                continue

        lerobot_dataset.finalize()


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
    from lerobot.datasets.utils import update_chunk_file_indices

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

    # Construct path using the same format as LeRobot
    video_path = lerobot_dataset.root / lerobot_dataset.meta.video_path.format(
        video_key=video_key, chunk_index=chunk_idx, file_index=file_idx
    )

    return video_path


def _apply_remuxed_videos(
    lerobot_dataset: LeRobotDataset,
    episode_index: int,
    remux_data: dict,
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
    import shutil
    import tempfile
    from pathlib import Path

    specs = remux_data["specs"]
    remux_info = remux_data["remux_info"]
    video_format = remux_data["video_format"]
    fps = remux_data["fps"]

    for spec in specs:
        if spec.key not in remux_info:
            continue

        info = remux_info[spec.key]
        video_key = f"observation.images.{spec.key}"

        # Construct the video file path ourselves (can't use get_video_file_path yet)
        video_path = _get_video_path_for_episode(lerobot_dataset, episode_index, video_key)

        # Check if the video file was created by LeRobot
        if not video_path.exists():
            print(f"    ⚠️  Video not found at {video_path}, skipping remux for '{spec.key}'")
            continue

        with tempfile.NamedTemporaryFile(suffix=".mp4", delete=False) as tmp_file:
            tmp_path = tmp_file.name

        try:
            remux_video_stream(
                samples=info["samples"],
                times_ns=info["times_ns"],
                output_path=tmp_path,
                video_format=video_format,
                target_fps=fps,
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
    data_columns: dict[str, list],
    num_rows: int,
    index_column: str,
    columns: ColumnSpec,
    action_dim: int | None,
    state_dim: int | None,
    task_default: str,
    fps: int,
    remux_data: dict,
) -> None:
    """
    Save an episode without decoding video frames by remuxing source packets directly.
    """
    import tempfile
    from pathlib import Path

    import numpy as np
    from lerobot.datasets.compute_stats import compute_episode_stats

    from .utils import normalize_times

    episode_index = lerobot_dataset.meta.total_episodes
    task_values = data_columns.get(columns.task, [None] * num_rows) if columns.task else [None] * num_rows

    tasks: list[str] = []
    for task_value in task_values:
        task_value = unwrap_singleton(task_value)
        if task_value is None:
            task = task_default
        elif isinstance(task_value, (bytes, bytearray, memoryview)):
            task = bytes(task_value).decode("utf-8")
        else:
            task = str(task_value)
        tasks.append(task)

    episode_tasks = list(dict.fromkeys(tasks))
    lerobot_dataset.meta.save_episode_tasks(episode_tasks)
    task_index = np.array([lerobot_dataset.meta.get_task_index(task) for task in tasks], dtype=np.int64)

    start_index = lerobot_dataset.meta.total_frames
    times_ns = normalize_times(data_columns[index_column])
    times_s = (times_ns - times_ns[0]) / 1_000_000_000.0
    episode_buffer: dict[str, np.ndarray] = {
        "timestamp": times_s.astype(np.float32),
        "frame_index": np.arange(num_rows, dtype=np.int64),
        "episode_index": np.full((num_rows,), episode_index, dtype=np.int64),
        "index": np.arange(start_index, start_index + num_rows, dtype=np.int64),
        "task_index": task_index,
    }

    if action_dim is not None and columns.action:
        action_values = [to_float32_vector(value, action_dim, "action") for value in data_columns[columns.action]]
        episode_buffer["action"] = np.stack(action_values)

    if state_dim is not None and columns.state:
        state_values = [to_float32_vector(value, state_dim, "state") for value in data_columns[columns.state]]
        episode_buffer["observation.state"] = np.stack(state_values)

    ep_stats = compute_episode_stats(
        episode_buffer,
        {key: lerobot_dataset.features[key] for key in episode_buffer},
    )

    episode_metadata = lerobot_dataset._save_episode_data(episode_buffer)

    video_metadata: dict[str, object] = {}
    specs = remux_data["specs"]
    remux_info = remux_data["remux_info"]
    video_format = remux_data["video_format"]
    fps = remux_data["fps"]

    for spec in specs:
        info = remux_info.get(spec.key)
        if info is None:
            continue

        video_key = f"observation.images.{spec.key}"
        temp_dir = Path(tempfile.mkdtemp(dir=lerobot_dataset.root))
        temp_path = temp_dir / f"{video_key.replace('.', '_')}_{episode_index:03d}.mp4"

        remux_video_stream(
            samples=info["samples"],
            times_ns=info["times_ns"],
            output_path=str(temp_path),
            video_format=video_format,
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


def _process_segment(
    dataset: rr.catalog.DatasetEntry,
    segment_id: str,
    index_column: str,
    columns: ColumnSpec,
    action_path: str | None,
    state_path: str | None,
    task_path: str | None,
    task_default: str,
    image_specs: list[ImageSpec],
    fps: int,
    features: dict[str, dict],
    lerobot_dataset: LeRobotDataset,
    video_format: str,
    use_videos: bool,
) -> tuple[bool, dict | None, bool]:
    """
    Process a single segment and add frames to the LeRobot dataset.

    Args:
        dataset: Rerun catalog dataset entry
        segment_id: ID of the segment to process
        index_column: Timeline column name
        columns: Column specifications
        action_path: Entity path for actions
        state_path: Entity path for state
        task_path: Entity path for task text
        task_default: Default task value
        image_specs: Image stream specifications
        fps: Target FPS
        features: Feature specifications from inference
        lerobot_dataset: LeRobot dataset to add frames to
        video_format: Video codec format
        use_videos: Whether videos are being used

    Returns:
        Tuple of (success, remux_data, direct_saved) where:
        - success: True if segment was processed successfully, False if skipped
        - remux_data: Dict with remuxing info if possible, None otherwise
        - direct_saved: True if the episode was saved without decoding video frames

    """
    # Check if segment is empty before processing
    # This can happen when RRD files contain multiple segments where some are empty
    # (e.g., after video processing creates a secondary empty segment)
    segment_table = dataset.segment_table()
    segment_info = pa.table(segment_table.df)
    for i in range(segment_info.num_rows):
        if segment_info["rerun_segment_id"][i].as_py() == segment_id:
            size_bytes = segment_info["rerun_size_bytes"][i].as_py()
            if size_bytes == 0:
                print(f"Skipping segment '{segment_id}': segment is empty (0 bytes)")
                return False, None, False
            break

    # Build list of entity paths (not full column specs) for filter_contents
    contents = []
    reference_path = None  # Path to use for establishing time range

    if action_path:
        # Extract entity path (part before first colon)
        entity_path = action_path.split(":")[0]
        if entity_path not in contents:
            contents.append(entity_path)
        if reference_path is None:
            reference_path = entity_path
    if state_path:
        entity_path = state_path.split(":")[0]
        if entity_path not in contents:
            contents.append(entity_path)
        if reference_path is None:
            reference_path = entity_path
    if task_path:
        entity_path = task_path.split(":")[0]
        if entity_path not in contents:
            contents.append(entity_path)
    for spec in image_specs:
        if spec.path not in contents:
            contents.append(spec.path)

    # Use a single reference path to establish time range (not all contents together)
    # This avoids getting empty results when contents don't perfectly overlap
    if reference_path is None:
        print(f"Skipping segment '{segment_id}': no action or state path specified")
        return False, None, False

    time_range_view = dataset.filter_segments(segment_id).filter_contents(reference_path)
    time_df = time_range_view.reader(index=index_column)

    # Get min/max times for this segment from the reference path
    min_max = time_df.aggregate(
        "rerun_segment_id",
        [F.min(col(index_column)).alias("min"), F.max(col(index_column)).alias("max")],
    )
    min_max_table = pa.table(min_max)

    # Check if we have any data in this segment
    if min_max_table.num_rows == 0 or min_max_table["min"][0] is None:
        print(
            f"Skipping segment '{segment_id}': no data on index '{index_column}' for reference path '{reference_path}'"
        )
        return False, None, False

    min_value = min_max_table["min"].to_numpy()[0]
    max_value = min_max_table["max"].to_numpy()[0]
    desired_times = make_time_grid(min_value, max_value, fps)

    # Now create a view with all contents and use the time grid to align them
    view = dataset.filter_segments(segment_id).filter_contents(contents)
    df = view.reader(index=index_column, using_index_values=desired_times, fill_latest_at=True)
    filters = []
    if columns.action:
        filters.append(col(columns.action).is_not_null())
    if columns.state:
        filters.append(col(columns.state).is_not_null())
    if filters:
        df = df.filter(*filters)

    action_dim = features["action"]["shape"][0] if "action" in features else None
    state_dim = features["observation.state"]["shape"][0] if "observation.state" in features else None

    # Load all video samples for the segment
    video_data_cache: dict[str, tuple[list[bytes], np.ndarray]] = {}
    for spec in image_specs:
        sample_column = f"{spec.path}:VideoStream:sample"
        video_view = dataset.filter_segments(segment_id).filter_contents(spec.path)
        video_reader = video_view.reader(index=index_column)
        video_table = pa.table(video_reader.select(index_column, sample_column))
        samples, times_ns = extract_video_samples(
            video_table, sample_column=sample_column, time_column=index_column
        )
        video_data_cache[spec.key] = (samples, times_ns)

    # Check if video remuxing is possible (when use_videos=True and FPS matches)
    remux_data = None
    if use_videos and image_specs:
        all_can_remux = True
        remux_info = {}
        for spec in image_specs:
            samples, times_ns = video_data_cache[spec.key]
            can_remux, source_fps = can_remux_video(times_ns, fps)
            if can_remux:
                remux_info[spec.key] = {
                    "samples": samples,
                    "times_ns": times_ns,
                    "source_fps": source_fps,
                }
                print(f"  ✓ Video '{spec.key}' can be remuxed (source: {source_fps:.2f}fps, target: {fps}fps)")
            else:
                all_can_remux = False
                print(f"  ✗ Video '{spec.key}' requires re-encoding (source: {source_fps:.2f}fps, target: {fps}fps)")

        if all_can_remux:
            remux_data = {
                "specs": image_specs,
                "remux_info": remux_info,
                "video_format": video_format,
                "fps": fps,
            }
            print(f"  🚀 Fast path: will remux {len(remux_info)} video(s) directly (100-1000x faster!)")
        else:
            print("  ⚠️  Slow path: will decode and re-encode videos (FPS mismatch)")

    table = pa.table(df)
    if table.num_rows == 0:
        return False, None, False

    data_columns = {name: table[name].to_pylist() for name in table.column_names}
    num_rows = table.num_rows

    if use_videos and remux_data:
        _save_episode_without_video_decode(
            lerobot_dataset=lerobot_dataset,
            data_columns=data_columns,
            num_rows=num_rows,
            index_column=index_column,
            columns=columns,
            action_dim=action_dim,
            state_dim=state_dim,
            task_default=task_default,
            fps=fps,
            remux_data=remux_data,
        )
        return True, None, True

    # Decode video frames for the entire segment if needed
    video_frames = _decode_video_frames_for_batch(
        table=table,
        index_column=index_column,
        image_specs=image_specs,
        video_data_cache=video_data_cache,
        video_format=video_format,
    )

    for row_idx in tqdm(range(num_rows), desc=f"Frames ({segment_id})", leave=False):
        frame = _build_frame(
            row_idx=row_idx,
            data_columns=data_columns,
            columns=columns,
            action_dim=action_dim,
            state_dim=state_dim,
            task_default=task_default,
            image_specs=image_specs,
            video_frames=video_frames,
            num_rows=num_rows,
        )
        lerobot_dataset.add_frame(frame)

    return True, remux_data, False


def _decode_video_frames_for_batch(
    table: pa.Table,
    index_column: str,
    image_specs: list[ImageSpec],
    video_data_cache: dict[str, tuple[list[bytes], np.ndarray]],
    video_format: str,
) -> dict[str, list[np.ndarray]]:
    """
    Decode video frames for a batch of rows.

    Args:
        table: PyArrow table containing the batch
        index_column: Timeline column name
        image_specs: Image stream specifications
        video_data_cache: Cached video data per spec key
        video_format: Video codec format

    Returns:
        Dictionary mapping spec key to list of decoded frames

    """
    from .utils import normalize_times

    video_frames: dict[str, list[np.ndarray]] = {}
    if video_data_cache:
        row_times_ns = normalize_times(table[index_column].to_pylist())
        for spec in image_specs:
            samples, times_ns = video_data_cache[spec.key]
            frames = []
            for time_ns in row_times_ns:
                frames.append(decode_video_frame(samples, times_ns, int(time_ns), video_format))
            video_frames[spec.key] = frames
    return video_frames


def _build_frame(
    row_idx: int,
    data_columns: dict[str, list],
    columns: ColumnSpec,
    action_dim: int | None,
    state_dim: int | None,
    task_default: str,
    image_specs: list[ImageSpec],
    video_frames: dict[str, list[np.ndarray]],
    num_rows: int,
) -> dict[str, object]:
    """
    Build a single frame dictionary for LeRobot dataset.

    Args:
        row_idx: Row index in the batch
        data_columns: Dictionary of column data
        columns: Column specifications
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

    # Add action if present
    if action_dim is not None and columns.action:
        frame["action"] = to_float32_vector(
            data_columns[columns.action][row_idx],
            action_dim,
            "action",
        )

    # Add state if present
    if state_dim is not None and columns.state:
        frame["observation.state"] = to_float32_vector(
            data_columns[columns.state][row_idx],
            state_dim,
            "state",
        )

    # Add task
    task_value = data_columns.get(columns.task, [None] * num_rows)[row_idx] if columns.task else None
    task_value = unwrap_singleton(task_value)
    if task_value is None:
        task = task_default
    elif isinstance(task_value, (bytes, bytearray, memoryview)):
        task = bytes(task_value).decode("utf-8")
    else:
        task = str(task_value)
    frame["task"] = task

    # Add video frames
    for spec in image_specs:
        image = video_frames[spec.key][row_idx]
        frame[f"observation.images.{spec.key}"] = image

    return frame
