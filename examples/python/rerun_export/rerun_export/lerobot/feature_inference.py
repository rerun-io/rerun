"""Feature shape inference for LeRobot datasets."""

from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np

from rerun_export.lerobot.video_processing import infer_video_shape_from_table

if TYPE_CHECKING:
    import pyarrow as pa

    from rerun_export.lerobot.types import FeatureSpec, LeRobotConversionConfig


def infer_features(
    *,
    table: pa.Table,
    config: LeRobotConversionConfig,
) -> dict[str, FeatureSpec]:
    """
    Infer feature specifications from a pre-queried PyArrow table.

    Args:
        table: PyArrow table containing all necessary columns (action, state, video samples)
        config: Conversion configuration

    Returns:
        Dictionary mapping feature names to their specifications

    Raises:
        ValueError: If features cannot be inferred or names don't match dimensions

    """
    features: dict[str, FeatureSpec] = {}

    # Infer action dimension
    if config.action not in table.column_names:
        raise ValueError(f"Action column '{config.action}' not found in table. Available columns: {table.column_names}")

    action_values = table[config.action].to_pylist()
    action_sample = next((v for v in action_values if v is not None), None)
    if action_sample is None:
        raise ValueError(f"Could not infer action dimension: no non-null values found in column '{config.action}'")

    action_dim = len(np.asarray(action_sample).flatten())
    if config.action_names is not None and len(config.action_names) != action_dim:
        raise ValueError("Action names length does not match inferred action dimension.")
    features["action"] = {"dtype": "float32", "shape": (action_dim,), "names": config.action_names}

    # Infer state dimension
    if config.state not in table.column_names:
        raise ValueError(f"State column '{config.state}' not found in table. Available columns: {table.column_names}")

    state_values = table[config.state].to_pylist()
    state_sample = next((v for v in state_values if v is not None), None)
    if state_sample is None:
        raise ValueError(f"Could not infer state dimension: no non-null values found in column '{config.state}'")

    state_dim = len(np.asarray(state_sample).flatten())
    if config.state_names is not None and len(config.state_names) != state_dim:
        raise ValueError("State names length does not match inferred state dimension.")
    features["observation.state"] = {"dtype": "float32", "shape": (state_dim,), "names": config.state_names}

    # Infer video shapes
    for spec in config.videos:
        sample_column = f"{spec['path']}:VideoStream:sample"
        video_format = spec.get("video_format", "h264")

        try:
            shape = infer_video_shape_from_table(
                table=table,
                sample_column=sample_column,
                index_column=config.index_column,
                video_format=video_format,
            )
        except ValueError as e:
            raise ValueError(f"Could not infer video shape for '{spec['path']}' (column '{sample_column}'): {e}") from e

        feature_key = f"observation.images.{spec['key']}"
        features[feature_key] = {
            "dtype": "video" if config.use_videos else "image",
            "shape": shape,
            "names": ["height", "width", "channels"],
        }

    return features
