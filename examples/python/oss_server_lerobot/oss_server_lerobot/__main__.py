#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path
from typing import TYPE_CHECKING

import pyarrow as pa
import rerun as rr
from datafusion import col, functions as F
from lerobot.datasets.lerobot_dataset import LeRobotDataset
from tqdm import tqdm

from .converter import convert_dataframe_to_episode
from .feature_inference import infer_features
from .types import ColumnSpec, ConversionConfig, ImageSpec
from .utils import make_time_grid
from .video_processing import extract_video_samples

if TYPE_CHECKING:
    import numpy as np


def _parse_image_specs(raw_specs: list[str]) -> list[ImageSpec]:
    specs: list[ImageSpec] = []
    for raw_spec in raw_specs:
        parts = raw_spec.split(":")
        if len(parts) != 2:
            raise ValueError("Image spec must be formatted as key:path (videostream only).")
        key, path = parts
        specs.append(ImageSpec(key=key, path=path))
    return specs


def _parse_name_list(raw: str | None) -> list[str] | None:
    if raw is None:
        return None
    names = [item.strip() for item in raw.split(",") if item.strip()]
    return names or None


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Convert a dataset of RRD recordings into a LeRobot v3 dataset.",
    )
    parser.add_argument("--rrd-dir", type=Path, required=True, help="Directory containing RRD recordings.")
    parser.add_argument("--output", type=Path, required=True, help="Output directory for the LeRobot dataset.")
    parser.add_argument("--dataset-name", default="rrd_dataset", help="Catalog dataset name.")
    parser.add_argument("--repo-id", default=None, help="LeRobot repo id (defaults to dataset name).")
    parser.add_argument("--fps", type=int, required=True, help="Target dataset FPS.")
    parser.add_argument("--index", default="real_time", help="Timeline to align on (e.g. real_time).")
    parser.add_argument("--task-default", default="task", help="Fallback task label when missing.")
    parser.add_argument("--action", default=None, help="Fully qualified action column (e.g. 'path:Component:field').")
    parser.add_argument("--state", default=None, help="Fully qualified state column (e.g. 'path:Component:field').")
    parser.add_argument("--task", default=None, help="Fully qualified task column (e.g. 'path:Component:field').")
    parser.add_argument(
        "--video",
        action="append",
        default=[],
        help="Video stream spec as key:path. Repeatable.",
    )
    parser.add_argument("--segments", nargs="*", default=None, help="Optional list of segment ids to convert.")
    parser.add_argument("--max-segments", type=int, default=None, help="Limit number of segments.")
    parser.add_argument("--use-images", action="store_true", help="Store images inline instead of videos.")
    parser.add_argument("--action-names", default=None, help="Comma-separated action names.")
    parser.add_argument("--state-names", default=None, help="Comma-separated state names.")
    parser.add_argument("--vcodec", default="libsvtav1", help="Video codec for LeRobot encoding.")
    parser.add_argument("--video-format", default="h264", help="Video stream codec format for decoding.")
    return parser.parse_args()


def convert_with_dataframes(
    rrd_dir: Path,
    output_dir: Path,
    dataset_name: str,
    repo_id: str,
    config: ConversionConfig,
    segments: list[str] | None = None,
    max_segments: int | None = None,
) -> None:
    """
    Convert RRD dataset to LeRobot using DataFusion dataframes.

    This function handles:
    1. Server connection and dataset access
    2. Feature inference
    3. Querying dataframes for each segment
    4. Calling the conversion function with dataframes

    Args:
        rrd_dir: Directory containing RRD recordings
        output_dir: Output directory for LeRobot dataset
        dataset_name: Catalog dataset name
        repo_id: LeRobot repo ID
        config: Conversion configuration
        segments: Optional list of segment IDs to convert
        max_segments: Optional limit on number of segments

    """

    if not rrd_dir.is_dir():
        raise ValueError(f"RRD directory does not exist or is not a directory: {rrd_dir}")
    if output_dir.exists():
        raise ValueError(f"Output directory already exists: {output_dir}")
    if config.columns.action is None and config.columns.state is None and not config.image_specs:
        raise ValueError("At least one of action_column, state_column, or image_specs must be provided.")

    with rr.server.Server(datasets={dataset_name: rrd_dir}) as server:
        client = server.client()
        dataset = client.get_dataset(name=dataset_name)
        segment_ids = list(segments) if segments else dataset.segment_ids()
        if max_segments is not None:
            segment_ids = segment_ids[:max_segments]
        if not segment_ids:
            raise ValueError("No segments found in the dataset.")

        # Infer features using the dataset (needs to probe multiple segments)
        features = infer_features(
            dataset=dataset,
            segment_id=segment_ids[0],
            index_column=config.index_column,
            columns=config.columns,
            image_specs=config.image_specs,
            use_videos=config.use_videos,
            action_names=config.action_names,
            state_names=config.state_names,
            video_format=config.video_format,
        )

        # Create LeRobot dataset
        lerobot_dataset = LeRobotDataset.create(
            repo_id=repo_id,
            fps=config.fps,
            features=features,
            root=output_dir,
            use_videos=config.use_videos,
            video_backend=config.vcodec,
        )

        # Process each segment
        for segment_id in tqdm(segment_ids, desc="Segments"):
            try:
                # Build content filter list from column names
                contents, reference_path = config.get_filter_list()

                if reference_path is None:
                    print(f"Skipping segment '{segment_id}': no action or state column specified")
                    continue

                # Check if segment is empty
                segment_table = dataset.segment_table()
                segment_info = pa.table(segment_table.df)
                is_empty = False
                for i in range(segment_info.num_rows):
                    if segment_info["rerun_segment_id"][i].as_py() == segment_id:
                        size_bytes = segment_info["rerun_size_bytes"][i].as_py()
                        if size_bytes == 0:
                            print(f"Skipping segment '{segment_id}': segment is empty (0 bytes)")
                            is_empty = True
                            break
                if is_empty:
                    continue

                # Get time range from reference path
                time_range_view = dataset.filter_segments(segment_id).filter_contents(reference_path)
                time_df = time_range_view.reader(index=config.index_column)
                min_max = time_df.aggregate(
                    "rerun_segment_id",
                    [F.min(col(config.index_column)).alias("min"), F.max(col(config.index_column)).alias("max")],
                )
                min_max_table = pa.table(min_max)

                if min_max_table.num_rows == 0 or min_max_table["min"][0] is None:
                    print(
                        f"Skipping segment '{segment_id}': no data on index '{config.index_column}' "
                        f"for reference path '{reference_path}'"
                    )
                    continue

                min_value = min_max_table["min"].to_numpy()[0]
                max_value = min_max_table["max"].to_numpy()[0]
                desired_times = make_time_grid(min_value, max_value, config.fps)

                # Query the dataframe with time alignment
                view = dataset.filter_segments(segment_id).filter_contents(contents)
                df = view.reader(index=config.index_column, using_index_values=desired_times, fill_latest_at=True)

                # Apply filters
                filters = []
                if config.columns.action:
                    filters.append(col(config.columns.action).is_not_null())
                if config.columns.state:
                    filters.append(col(config.columns.state).is_not_null())
                if filters:
                    df = df.filter(*filters)

                # Load video data cache

                video_data_cache: dict[str, tuple[list[bytes], np.ndarray]] = {}
                for spec in config.image_specs:
                    sample_column = f"{spec.path}:VideoStream:sample"
                    video_view = dataset.filter_segments(segment_id).filter_contents(spec.path)
                    video_reader = video_view.reader(index=config.index_column)
                    video_table = pa.table(video_reader.select(config.index_column, sample_column))
                    samples, times_ns = extract_video_samples(
                        video_table,
                        sample_column=sample_column,
                        time_column=config.index_column,
                    )
                    video_data_cache[spec.key] = (samples, times_ns)

                # Convert the dataframe to an episode
                success, remux_data, direct_saved = convert_dataframe_to_episode(
                    df=df,
                    config=config,
                    video_data_cache=video_data_cache,
                    lerobot_dataset=lerobot_dataset,
                    segment_id=segment_id,
                    features=features,
                )

                if success and not direct_saved:
                    episode_index = lerobot_dataset.episode_buffer["episode_index"]
                    lerobot_dataset.save_episode()

                    # Apply remuxed videos if possible
                    if config.use_videos and remux_data:
                        from .converter import _apply_remuxed_videos

                        _apply_remuxed_videos(lerobot_dataset, episode_index, remux_data)

            except Exception as e:
                print(f"Error processing segment {segment_id}: {e}")
                import traceback

                traceback.print_exc()
                continue

        lerobot_dataset.finalize()


def main() -> None:
    args = _parse_args()
    image_specs = _parse_image_specs(args.video)
    repo_id = args.repo_id or args.dataset_name

    columns = ColumnSpec(action=args.action, state=args.state, task=args.task)

    config = ConversionConfig(
        fps=args.fps,
        index_column=args.index,
        columns=columns,
        image_specs=image_specs,
        use_videos=not args.use_images,
        video_format=args.video_format,
        vcodec=args.vcodec,
        action_names=_parse_name_list(args.action_names),
        state_names=_parse_name_list(args.state_names),
        task_default=args.task_default,
    )

    convert_with_dataframes(
        rrd_dir=args.rrd_dir,
        output_dir=args.output,
        dataset_name=args.dataset_name,
        repo_id=repo_id,
        config=config,
        segments=args.segments,
        max_segments=args.max_segments,
    )


if __name__ == "__main__":
    main()
