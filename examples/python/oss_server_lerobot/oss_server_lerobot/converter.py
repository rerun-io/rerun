"""Main conversion logic for RRD to LeRobot dataset conversion."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import rerun as rr
from datafusion import col, functions as F
from tqdm import tqdm

from .feature_inference import infer_features
from .types import ColumnSpec, ImageSpec
from .utils import make_time_grid, to_float32_vector, unwrap_singleton
from .video_processing import decode_video_frame, extract_video_samples

if TYPE_CHECKING:
    from pathlib import Path
    import numpy as np
    from collections.abc import Iterable
    from lerobot.datasets.lerobot_dataset import LeRobotDataset


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
        client = rr.catalog.CatalogClient(server.address())
        dataset = client.get_dataset(name=dataset_name)
        segment_ids = list(segments) if segments else dataset.segment_ids()
        if max_segments is not None:
            segment_ids = segment_ids[:max_segments]
        if not segment_ids:
            raise ValueError("No segments found in the dataset.")

        features = infer_features(
            dataset,
            segment_ids[0],
            index_column,
            columns,
            image_specs,
            use_videos,
            action_names,
            state_names,
            video_format,
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
                success = _process_segment(
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
                )
                if success:
                    lerobot_dataset.save_episode()

            except Exception as e:
                print(f"Error processing segment {segment_id}: {e}")
                import traceback

                traceback.print_exc()
                continue

        lerobot_dataset.finalize()


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
) -> bool:
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

    Returns:
        True if segment was processed successfully, False if skipped

    """
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
        return False

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
            f"Skipping segment '{segment_id}': no data on index '{index_column}' "
            f"for reference path '{reference_path}'"
        )
        return False

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

    # Process in batches to avoid loading entire segment into memory at once
    BATCH_SIZE = 1000  # Process 1000 rows at a time
    batch_offset = 0
    action_dim = features["action"]["shape"][0] if "action" in features else None
    state_dim = features["observation.state"]["shape"][0] if "observation.state" in features else None

    # Load all video samples for the segment since video decoding requires access to keyframes
    video_data_cache: dict[str, tuple[list[bytes], np.ndarray, np.ndarray]] = {}
    for spec in image_specs:
        sample_column = f"{spec.path}:VideoStream:sample"
        keyframe_column = f"{spec.path}:is_keyframe"
        video_view = dataset.filter_segments(segment_id).filter_contents(spec.path)
        video_table = pa.table(video_view.reader(index=index_column).select(index_column, sample_column))
        samples, times_ns, keyframes = extract_video_samples(video_table, sample_column, keyframe_column, index_column)
        video_data_cache[spec.key] = (samples, times_ns, keyframes)

    while True:
        # Get a batch of data
        batch_df = df.limit(BATCH_SIZE, offset=batch_offset)
        table = pa.table(batch_df)

        if table.num_rows == 0:
            break

        data_columns = {name: table[name].to_pylist() for name in table.column_names}
        num_rows = table.num_rows

        # Decode video frames for this batch if needed
        video_frames = _decode_video_frames_for_batch(
            table=table,
            index_column=index_column,
            image_specs=image_specs,
            video_data_cache=video_data_cache,
            video_format=video_format,
        )

        for row_idx in tqdm(range(num_rows), desc=f"Frames ({segment_id}, batch {batch_offset})", leave=False):
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

        batch_offset += num_rows

        # If we got fewer rows than BATCH_SIZE, we've reached the end
        if num_rows < BATCH_SIZE:
            break

    return True


def _decode_video_frames_for_batch(
    table: pa.Table,
    index_column: str,
    image_specs: list[ImageSpec],
    video_data_cache: dict[str, tuple[list[bytes], np.ndarray, np.ndarray]],
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
            samples, times_ns, keyframes = video_data_cache[spec.key]
            frames = []
            for time_ns in row_times_ns:
                frames.append(decode_video_frame(samples, times_ns, keyframes, int(time_ns), video_format))
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
