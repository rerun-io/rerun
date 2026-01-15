"""Feature shape inference for LeRobot datasets."""

from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa

from .video_processing import infer_video_shape

if TYPE_CHECKING:
    import rerun as rr
    from datafusion import DataFrame as DataFusionDataFrame

    from .types import ColumnSpec, ImageSpec


def infer_features(
    *,
    dataset: rr.catalog.DatasetEntry,
    segment_id: str,
    index_column: str,
    columns: ColumnSpec,
    image_specs: list[ImageSpec],
    use_videos: bool,
    action_names: list[str] | None,
    state_names: list[str] | None,
    video_format: str,
) -> dict[str, dict]:
    """
    Infer feature specifications for a LeRobot dataset.

    Args:
        dataset: Rerun catalog dataset entry
        segment_id: ID of a segment to sample from
        index_column: Name of the index/timeline column
        columns: Column specifications for action, state, and task
        image_specs: Image stream specifications
        use_videos: Whether to use video encoding
        action_names: Optional names for action dimensions
        state_names: Optional names for state dimensions
        video_format: Video codec format

    Returns:
        Dictionary mapping feature names to their specifications

    Raises:
        ValueError: If features cannot be inferred or names don't match dimensions

    """
    # Build content filter list (entity paths only, not full column specs)
    contents = []
    if columns.action:
        action_path = columns.action.split(":")[0]
        if action_path not in contents:
            contents.append(action_path)
    if columns.state:
        state_path = columns.state.split(":")[0]
        if state_path not in contents:
            contents.append(state_path)
    if columns.task:
        task_path = columns.task.split(":")[0]
        if task_path not in contents:
            contents.append(task_path)
    for spec in image_specs:
        if spec.path not in contents:
            contents.append(spec.path)

    columns_to_read = [index_column]
    if columns.action:
        columns_to_read.append(columns.action)

    # TODO(gijsd): do we want to handle this like this?
    if columns.state and columns.state != columns.action:  # Avoid duplicates
        columns_to_read.append(columns.state)
    if columns.task:
        columns_to_read.append(columns.task)

    if columns.action:
        action_col_exists = columns.action in columns_to_read
        print(f"Action column '{columns.action}' exists: {action_col_exists}")

    features = {}

    # Infer action and state dimensions by trying multiple segments
    segments_to_try = [segment_id] + [s for s in dataset.segment_ids() if s != segment_id]

    if columns.action:
        action_dim = None
        for try_segment_id in segments_to_try:
            try:
                try_view = dataset.filter_segments(try_segment_id).filter_contents(contents)
                try_df = try_view.reader(index=index_column).select_columns(index_column, columns.action)
                try_table = try_df.to_pydict()
                action_values = try_table.get(columns.action, [])
                if action_values:
                    action_sample = next((v for v in action_values if v is not None), None)
                    if action_sample is not None:
                        action_dim = len(np.asarray(action_sample).flatten())
            except Exception:
                continue

        if action_dim is None:
            raise ValueError(
                f"Could not infer action dimension from any segment. Tried {len(segments_to_try)} segments."
            )
        if action_names is not None and len(action_names) != action_dim:
            raise ValueError("Action names length does not match inferred action dimension.")
        features["action"] = {"dtype": "float32", "shape": (action_dim,), "names": action_names}

    if columns.state:
        state_dim = None
        for try_segment_id in segments_to_try:
            try:
                try_view = dataset.filter_segments(try_segment_id).filter_contents(contents)
                try_df = try_view.reader(index=index_column).select_columns(index_column, columns.state)
                try_table = try_df.to_pydict()
                state_values = try_table.get(columns.state, [])
                if state_values:
                    state_sample = next((v for v in state_values if v is not None), None)
                    if state_sample is not None:
                        state_dim = len(np.asarray(state_sample).flatten())
                        break
            except Exception:
                continue

        if state_dim is None:
            raise ValueError(
                f"Could not infer state dimension from any segment. Tried {len(segments_to_try)} segments."
            )
        if state_names is not None and len(state_names) != state_dim:
            raise ValueError("State names length does not match inferred state dimension.")
        features["observation.state"] = {"dtype": "float32", "shape": (state_dim,), "names": state_names}

    for spec in image_specs:
        # Video specs need to find a segment with actual video data on the specified index
        # Try the current segment first, then try other segments if needed
        shape = None
        segments_to_try = [segment_id] + [s for s in dataset.segment_ids() if s != segment_id]
        for try_segment_id in segments_to_try:
            try:
                shape = infer_video_shape(dataset, try_segment_id, index_column, spec, video_format)
                break
            except ValueError as e:
                # This segment doesn't have video data, try the next one
                if try_segment_id == segments_to_try[-1]:
                    # Last segment, re-raise the error
                    raise ValueError(
                        f"Could not find any segment with video data for '{spec.path}' "
                        f"using index '{index_column}'. Tried {len(segments_to_try)} segments."
                    ) from e
                continue
        features[f"observation.images.{spec.key}"] = {
            "dtype": "video" if use_videos else "image",
            "shape": shape,
            "names": ["height", "width", "channels"],
        }

    return features


def infer_features_from_dataframe(
    *,
    df: DataFusionDataFrame,
    columns: ColumnSpec,
    image_shapes: dict[str, tuple[int, int, int]],
    use_videos: bool,
    action_names: list[str] | None,
    state_names: list[str] | None,
) -> dict[str, dict]:
    """
    Infer feature specifications from a DataFusion dataframe.

    This is useful when you already have a queried dataframe and want to infer
    features from it without re-querying the dataset.

    Args:
        df: DataFusion dataframe containing sample data
        columns: Column specifications for action, state, and task
        image_shapes: Pre-computed image shapes for each image key (height, width, channels)
        use_videos: Whether to use video encoding
        action_names: Optional names for action dimensions
        state_names: Optional names for state dimensions

    Returns:
        Dictionary mapping feature names to their specifications

    Raises:
        ValueError: If features cannot be inferred or names don't match dimensions

    """
    features = {}

    # Convert dataframe to PyArrow table to access data
    table = pa.table(df)

    # Infer action dimension
    if columns.action and columns.action in table.column_names:
        action_values = table[columns.action].to_pylist()
        action_sample = next((v for v in action_values if v is not None), None)
        if action_sample is not None:
            action_dim = len(np.asarray(action_sample).flatten())
            if action_names is not None and len(action_names) != action_dim:
                raise ValueError("Action names length does not match inferred action dimension.")
            features["action"] = {"dtype": "float32", "shape": (action_dim,), "names": action_names}

    # Infer state dimension
    if columns.state and columns.state in table.column_names:
        state_values = table[columns.state].to_pylist()
        state_sample = next((v for v in state_values if v is not None), None)
        if state_sample is not None:
            state_dim = len(np.asarray(state_sample).flatten())
            if state_names is not None and len(state_names) != state_dim:
                raise ValueError("State names length does not match inferred state dimension.")
            features["observation.state"] = {"dtype": "float32", "shape": (state_dim,), "names": state_names}

    # Add image features using pre-computed shapes
    for key, shape in image_shapes.items():
        features[f"observation.images.{key}"] = {
            "dtype": "video" if use_videos else "image",
            "shape": shape,
            "names": ["height", "width", "channels"],
        }

    return features
