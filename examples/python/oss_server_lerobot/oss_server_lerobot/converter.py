from __future__ import annotations

from dataclasses import dataclass
from fractions import Fraction
from io import BytesIO
from pathlib import Path
from typing import Iterable

import av
import numpy as np
import pyarrow as pa
import rerun as rr
from datafusion import col
from datafusion import functions as F
from lerobot.datasets.lerobot_dataset import LeRobotDataset
from PIL import Image
from tqdm import tqdm


@dataclass(frozen=True)
class ImageSpec:
    key: str
    path: str
    kind: str  # "raw" | "compressed" | "video"


@dataclass(frozen=True)
class ColumnSpec:
    action: str | None
    state: str | None
    task: str | None


def _unwrap_singleton(value: object) -> object:
    if isinstance(value, list) and len(value) == 1:
        return value[0]
    if isinstance(value, np.ndarray) and value.shape[:1] == (1,):
        return value[0]
    return value


def _to_float32_vector(value: object, expected_dim: int, label: str) -> np.ndarray:
    if value is None:
        raise ValueError(f"Missing {label} value.")
    value = _unwrap_singleton(value)
    array = np.asarray(value, dtype=np.float32)
    if array.ndim == 0:
        array = array.reshape(1)
    if array.ndim == 2 and array.shape[0] == 1:
        array = array[0]
    if array.shape[0] != expected_dim:
        raise ValueError(f"{label} has dim {array.shape[0]} but expected {expected_dim}.")
    return array


def _decode_raw_image(buffer_value: object, format_value: object) -> np.ndarray:
    buffer_value = _unwrap_singleton(buffer_value)
    format_value = _unwrap_singleton(format_value)
    if buffer_value is None or format_value is None:
        raise ValueError("Missing raw image buffer or format.")

    flattened = np.asarray(buffer_value)
    format_details = format_value
    if isinstance(format_details, dict):
        height = int(format_details["height"])
        width = int(format_details["width"])
        color_model = int(format_details["color_model"])
    else:
        raise ValueError("Raw image format details are missing required fields.")

    num_channels = rr.datatypes.color_model.ColorModel.auto(color_model).num_channels()
    return flattened.reshape(height, width, num_channels)


def _decode_compressed_image(blob_value: object) -> np.ndarray:
    blob_value = _unwrap_singleton(blob_value)
    if blob_value is None:
        raise ValueError("Missing compressed image blob.")
    image = Image.open(BytesIO(bytes(blob_value)))
    return np.asarray(image)


def _infer_feature_shape(table: pa.Table, column: str, label: str) -> int:
    if column not in table.column_names:
        raise ValueError(f"Column {column} for {label} was not found.")
    values = table[column].to_pylist()

    for value in values:
        if value is None:
            continue
        array = np.asarray(_unwrap_singleton(value))
        if array.ndim == 0:
            return 1
        if array.ndim == 2 and array.shape[0] == 1:
            return int(array.shape[1])
        return int(array.shape[0])
    raise ValueError(f"Unable to infer {label} dimension; column {column} contains only nulls.")


def _infer_image_shape(table: pa.Table, spec: ImageSpec) -> tuple[int, int, int]:
    if spec.kind == "raw":
        buffer_column = f"{spec.path}:Image:buffer"
        format_column = f"{spec.path}:Image:format"
        if buffer_column not in table.column_names or format_column not in table.column_names:
            raise ValueError(f"Missing raw image columns for {spec.key}.")
        buffers = table[buffer_column].to_pylist()
        formats = table[format_column].to_pylist()
        for buffer_value, format_value in zip(buffers, formats, strict=False):
            if buffer_value is None or format_value is None:
                continue
            decoded = _decode_raw_image(buffer_value, format_value)
            return decoded.shape
        raise ValueError(f"Unable to infer raw image shape for {spec.key}.")

    if spec.kind == "compressed":
        blob_column = f"{spec.path}:EncodedImage:blob"
        if blob_column not in table.column_names:
            raise ValueError(f"Missing compressed image column for {spec.key}.")
        blobs = table[blob_column].to_pylist()
        for blob_value in blobs:
            if blob_value is None:
                continue
            decoded = _decode_compressed_image(blob_value)
            return decoded.shape
        raise ValueError(f"Unable to infer compressed image shape for {spec.key}.")

    raise ValueError(f"Unsupported image kind '{spec.kind}' for {spec.key}.")


def _normalize_times(values: Iterable[object]) -> np.ndarray:
    times = np.asarray(list(values))
    if np.issubdtype(times.dtype, np.datetime64):
        return times.astype("datetime64[ns]").astype("int64")
    if np.issubdtype(times.dtype, np.timedelta64):
        return times.astype("timedelta64[ns]").astype("int64")
    if np.issubdtype(times.dtype, np.floating):
        return (times * 1_000_000_000.0).astype("int64")
    return times.astype("int64")


def _extract_video_samples(
    table: pa.Table, sample_column: str, keyframe_column: str, time_column: str
) -> tuple[list[bytes], np.ndarray, np.ndarray]:
    samples_raw = table[sample_column].to_pylist()
    keyframes_raw = (
        table[keyframe_column].to_pylist() if keyframe_column in table.column_names else [None] * len(samples_raw)
    )
    times_raw = table[time_column].to_pylist()
    samples: list[bytes] = []
    keyframes: list[bool] = []
    times: list[object] = []
    for sample, keyframe, timestamp in zip(samples_raw, keyframes_raw, times_raw, strict=False):
        sample = _unwrap_singleton(sample)
        if sample is None:
            continue
        if isinstance(sample, np.ndarray):
            sample_bytes = sample.tobytes()
        else:
            sample_bytes = bytes(sample)
        samples.append(sample_bytes)
        keyframes.append(bool(_unwrap_singleton(keyframe)) if keyframe is not None else False)
        times.append(timestamp)
    if not samples:
        raise ValueError("No video samples available for decoding.")
    return samples, _normalize_times(times), np.asarray(keyframes, dtype=bool)


def _decode_video_frame(
    samples: list[bytes],
    times_ns: np.ndarray,
    keyframes: np.ndarray,
    target_time_ns: int,
    video_format: str,
) -> np.ndarray:
    idx = int(np.searchsorted(times_ns, target_time_ns, side="right") - 1)
    if idx < 0:
        idx = 0

    keyframe_indices = np.where(keyframes)[0]
    if keyframe_indices.size == 0:
        keyframe_idx = 0
    else:
        kf_pos = np.searchsorted(keyframe_indices, idx, side="right") - 1
        keyframe_idx = int(keyframe_indices[max(kf_pos, 0)])

    sample_bytes = b"".join(samples[keyframe_idx : idx + 1])
    data_buffer = BytesIO(sample_bytes)
    container = av.open(data_buffer, format=video_format, mode="r")
    video_stream = container.streams.video[0]
    start_time = times_ns[keyframe_idx]
    latest_frame = None
    packet_times = times_ns[keyframe_idx : idx + 1]
    for packet, time_ns in zip(container.demux(video_stream), packet_times, strict=False):
        packet.time_base = Fraction(1, 1_000_000_000)
        packet.pts = int(time_ns - start_time)
        packet.dts = packet.pts
        for frame in packet.decode():
            latest_frame = frame
    if latest_frame is None:
        raise ValueError("Failed to decode video frame for target time.")
    return np.asarray(latest_frame.to_image())


def _infer_video_shape(
    dataset: rr.catalog.DatasetEntry,
    segment_id: str,
    index_column: str,
    spec: ImageSpec,
    video_format: str,
) -> tuple[int, int, int]:
    view = dataset.filter_segments(segment_id).filter_contents(spec.path)
    sample_column = f"{spec.path}:VideoStream:sample"
    keyframe_column = f"{spec.path}:is_keyframe"
    df = view.reader(index=index_column).select(index_column, sample_column, keyframe_column)
    table = pa.table(df)
    samples, times_ns, keyframes = _extract_video_samples(table, sample_column, keyframe_column, index_column)
    target_time_ns = int(times_ns[0])
    decoded = _decode_video_frame(samples, times_ns, keyframes, target_time_ns, video_format)
    return decoded.shape


def _make_time_grid(min_value: object, max_value: object, fps: int) -> np.ndarray:
    min_array = np.asarray(min_value)
    if np.issubdtype(min_array.dtype, np.datetime64):
        step = np.timedelta64(int(1_000_000_000 / fps), "ns")
        if max_value <= min_value:
            return np.array([min_value])
        return np.arange(min_value, max_value, step)
    if max_value <= min_value:
        return np.array([min_value], dtype=np.float64)
    return np.arange(float(min_value), float(max_value), 1.0 / fps)


def _infer_features(
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
    # Build content filter list (entity paths) - same as main conversion loop
    contents = []
    if columns.action:
        action_path = columns.action.split(":")[0]
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

    # Filter contents BEFORE creating reader
    # TODO(gijsd): is this required?!
    view = dataset.filter_segments(segment_id).filter_contents(contents)
    print("view contents:", view.schema().column_names())

    columns_to_read = [index_column]
    if columns.action:
        columns_to_read.append(columns.action)

    # TODO(gijsd): do we want to handle this like this?
    if columns.state and columns.state != columns.action:  # Avoid duplicates
        columns_to_read.append(columns.state)
    if columns.task:
        columns_to_read.append(columns.task)
    for spec in image_specs:
        if spec.kind == "raw":
            columns_to_read.append(f"{spec.path}:Image:buffer")
            columns_to_read.append(f"{spec.path}:Image:format")
        elif spec.kind == "compressed":
            columns_to_read.append(f"{spec.path}:EncodedImage:blob")

    if columns.action:
        action_col_exists = columns.action in columns_to_read
        print(f"Action column '{columns.action}' exists: {action_col_exists}")

    print("segment_id:", segment_id)
    print("index_column:", index_column)
    print("columns_to_read:", columns_to_read)
    print("schema:", view.schema())
    print("reader for index:", view.reader(index=index_column).schema())
    df = view.reader(index=index_column).select_columns(*columns_to_read).limit(10)
    print("raw df:", df)
    print("df collect:", df.collect())
    table = df.to_arrow_table()

    features = {}
    if columns.action:
        action_dim = _infer_feature_shape(table, columns.action, "action")
        if action_names is not None and len(action_names) != action_dim:
            raise ValueError("Action names length does not match inferred action dimension.")
        features["action"] = {"dtype": "float32", "shape": (action_dim,), "names": action_names}

    if columns.state:
        state_dim = _infer_feature_shape(table, columns.state, "state")
        if state_names is not None and len(state_names) != state_dim:
            raise ValueError("State names length does not match inferred state dimension.")
        features["observation.state"] = {"dtype": "float32", "shape": (state_dim,), "names": state_names}

    for spec in image_specs:
        if spec.kind == "video":
            shape = _infer_video_shape(dataset, segment_id, index_column, spec, video_format)
        else:
            shape = _infer_image_shape(table, spec)
        features[f"observation.images.{spec.key}"] = {
            "dtype": "video" if use_videos else "image",
            "shape": shape,
            "names": ["height", "width", "channels"],
        }

    return features


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

        features = _infer_features(
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
            vcodec=vcodec,
        )

        # Process each segment (recording) separately
        for segment_id in tqdm(segment_ids, desc="Segments"):
            try:
                contents = []
                if action_path:
                    contents.append(action_path)
                if state_path:
                    contents.append(state_path)
                if task_path:
                    contents.append(task_path)
                for spec in image_specs:
                    contents.append(spec.path)

                view = dataset.filter_segments(segment_id).filter_contents(contents)
                df = view.reader(index=index_column)

                # Get min/max times for this segment
                min_max = df.aggregate(
                    "rerun_segment_id",
                    [F.min(col(index_column)).alias("min"), F.max(col(index_column)).alias("max")],
                )
                min_max_table = pa.table(min_max)
                min_value = min_max_table["min"].to_numpy()[0]
                max_value = min_max_table["max"].to_numpy()[0]
                desired_times = _make_time_grid(min_value, max_value, fps)

                df = view.reader(index=index_column, using_index_values=desired_times, fill_latest_at=True)
                filters = []
                if columns.action:
                    filters.append(col(columns.action).is_not_null())
                if columns.state:
                    filters.append(col(columns.state).is_not_null())
                for spec in image_specs:
                    if spec.kind == "raw":
                        filters.append(col(f"{spec.path}:Image:buffer").is_not_null())
                        filters.append(col(f"{spec.path}:Image:format").is_not_null())
                    elif spec.kind == "compressed":
                        filters.append(col(f"{spec.path}:EncodedImage:blob").is_not_null())
                    elif spec.kind == "video":
                        pass
                if filters:
                    df = df.filter(*filters)

                # Process in batches to avoid loading entire segment into memory at once
                BATCH_SIZE = 1000  # Process 1000 rows at a time
                batch_offset = 0
                action_dim = features["action"]["shape"][0] if "action" in features else None
                state_dim = features["observation.state"]["shape"][0] if "observation.state" in features else None

                # For video streams, we still need to load all samples for the segment
                # since video decoding requires access to keyframes
                video_data_cache: dict[str, tuple[list[bytes], np.ndarray, np.ndarray]] = {}
                for spec in image_specs:
                    if spec.kind != "video":
                        continue
                    sample_column = f"{spec.path}:VideoStream:sample"
                    keyframe_column = f"{spec.path}:is_keyframe"
                    video_view = dataset.filter_segments(segment_id).filter_contents(spec.path)
                    video_table = pa.table(
                        video_view.reader(index=index_column).select(index_column, sample_column, keyframe_column)
                    )
                    samples, times_ns, keyframes = _extract_video_samples(
                        video_table, sample_column, keyframe_column, index_column
                    )
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
                    video_frames: dict[str, list[np.ndarray]] = {}
                    if video_data_cache:
                        row_times_ns = _normalize_times(table[index_column].to_pylist())
                        for spec in image_specs:
                            if spec.kind != "video":
                                continue
                            samples, times_ns, keyframes = video_data_cache[spec.key]
                            frames = []
                            for time_ns in row_times_ns:
                                frames.append(
                                    _decode_video_frame(samples, times_ns, keyframes, int(time_ns), video_format)
                                )
                            video_frames[spec.key] = frames

                    for row_idx in tqdm(
                        range(num_rows), desc=f"Frames ({segment_id}, batch {batch_offset})", leave=False
                    ):
                        frame: dict[str, object] = {}
                        if action_dim is not None and columns.action:
                            frame["action"] = _to_float32_vector(
                                data_columns[columns.action][row_idx],
                                action_dim,
                                "action",
                            )
                        if state_dim is not None and columns.state:
                            frame["observation.state"] = _to_float32_vector(
                                data_columns[columns.state][row_idx],
                                state_dim,
                                "state",
                            )

                        task_value = (
                            data_columns.get(columns.task, [None] * num_rows)[row_idx] if columns.task else None
                        )
                        task_value = _unwrap_singleton(task_value)
                        if task_value is None:
                            task = task_default
                        elif isinstance(task_value, (bytes, bytearray, memoryview)):
                            task = bytes(task_value).decode("utf-8")
                        else:
                            task = str(task_value)
                        frame["task"] = task

                        for spec in image_specs:
                            if spec.kind == "raw":
                                buffer_column = f"{spec.path}:Image:buffer"
                                format_column = f"{spec.path}:Image:format"
                                image = _decode_raw_image(
                                    data_columns[buffer_column][row_idx],
                                    data_columns[format_column][row_idx],
                                )
                            elif spec.kind == "compressed":
                                blob_column = f"{spec.path}:EncodedImage:blob"
                                image = _decode_compressed_image(data_columns[blob_column][row_idx])
                            elif spec.kind == "video":
                                image = video_frames[spec.key][row_idx]
                            else:
                                raise ValueError(f"Unsupported image kind '{spec.kind}'.")
                            frame[f"observation.images.{spec.key}"] = image

                        lerobot_dataset.add_frame(frame)

                    batch_offset += num_rows

                    # If we got fewer rows than BATCH_SIZE, we've reached the end
                    if num_rows < BATCH_SIZE:
                        break

                lerobot_dataset.save_episode()

            except Exception as e:
                print(f"Error processing segment {segment_id}: {e}")
                import traceback

                traceback.print_exc()
                continue

        lerobot_dataset.finalize()
