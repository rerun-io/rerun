#!/usr/bin/env python3
from __future__ import annotations

import argparse
import time
from pathlib import Path

import pyarrow as pa
import rerun as rr
from lerobot.datasets.lerobot_dataset import LeRobotDataset  # type: ignore[import-untyped,import-not-found]
from tqdm import tqdm

from rerun_export.lerobot.converter import convert_dataframe_to_episode
from rerun_export.lerobot.feature_inference import infer_features
from rerun_export.lerobot.types import LeRobotConversionConfig, VideoSpec


def _parse_video_specs(raw_specs: list[str]) -> list[VideoSpec]:
    specs: list[VideoSpec] = []
    for raw_spec in raw_specs:
        parts = raw_spec.split(":")
        if len(parts) != 2:
            raise ValueError("Video spec must be formatted as key:path (videostream only).")
        key, path = parts
        specs.append(VideoSpec(key=key, path=path))
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
    return parser.parse_args()


def convert_rrd_dataset_to_lerobot(
    *,
    rrd_dir: Path,
    output_dir: Path,
    dataset_name: str,
    repo_id: str,
    config: LeRobotConversionConfig,
    segments: list[str] | None = None,
    max_segments: int | None = None,
) -> None:
    """
    Convert RRD dataset to LeRobot using the OSS server.

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

    with rr.server.Server(datasets={dataset_name: rrd_dir}) as server:
        client = server.client()
        dataset = client.get_dataset(name=dataset_name)
        segment_ids = list(segments) if segments else dataset.segment_ids()
        if max_segments is not None:
            segment_ids = segment_ids[:max_segments]
        if not segment_ids:
            raise ValueError("No segments found in the dataset.")

        # Query a representative segment for feature inference
        inference_segment_id = segment_ids[0]
        contents, reference_path = config.get_filter_list()

        # Build list of all columns needed for feature inference
        inference_columns = [config.index_column, config.action, config.state]
        if config.task:
            inference_columns.append(config.task)
        for spec in config.videos:
            inference_columns.append(f"{spec['path']}:VideoStream:sample")

        # Query all columns from one segment
        inference_view = dataset.filter_segments(inference_segment_id).filter_contents(contents)
        inference_reader = inference_view.reader(index=config.index_column)
        inference_table = pa.table(inference_reader.select(*inference_columns))

        print("Inferring features from segment:", inference_segment_id)
        start_time = time.time()
        # Infer features from the pre-queried table
        features = infer_features(
            table=inference_table,
            config=config,
        )
        end_time = time.time()
        print(f"Inferring features took {end_time - start_time:.2f} seconds")

        # Create LeRobot dataset
        lerobot_dataset = LeRobotDataset.create(
            repo_id=repo_id,
            fps=config.fps,
            features=features,
            root=output_dir,
            use_videos=config.use_videos,
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

                view = dataset.filter_segments(segment_id).filter_contents(contents)
                df = view.reader(
                    index=config.index_column,
                )

                # Convert the dataframe to an episode
                convert_dataframe_to_episode(
                    df,
                    config,
                    lerobot_dataset=lerobot_dataset,
                    segment_id=segment_id,
                    features=features,
                )

            except Exception as e:
                print(f"Error processing segment {segment_id}: {e}")
                import traceback

                traceback.print_exc()
                continue

        lerobot_dataset.finalize()


def main() -> None:
    args = _parse_args()
    video_specs = _parse_video_specs(args.video)
    repo_id = args.repo_id or args.dataset_name

    if args.action is None or args.state is None:
        raise ValueError("--action and --state must be provided.")

    config = LeRobotConversionConfig(
        fps=args.fps,
        index_column=args.index,
        action=args.action,
        state=args.state,
        task=args.task,
        videos=video_specs,
        use_videos=not args.use_images,
        action_names=_parse_name_list(args.action_names),
        state_names=_parse_name_list(args.state_names),
    )

    convert_rrd_dataset_to_lerobot(
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
