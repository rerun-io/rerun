#!/usr/bin/env python3
from __future__ import annotations

import argparse
from pathlib import Path

from .converter import convert_rrd_dataset_to_lerobot
from .types import ImageSpec


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
    parser.add_argument("--action-path", default=None, help="Rerun entity path for actions.")
    parser.add_argument("--state-path", default=None, help="Rerun entity path for state observations.")
    parser.add_argument("--task-path", default=None, help="Rerun entity path for task text.")
    parser.add_argument("--task-default", default="task", help="Fallback task label when missing.")
    parser.add_argument("--action-column", default=None, help="Override action column name.")
    parser.add_argument("--state-column", default=None, help="Override state column name.")
    parser.add_argument("--task-column", default=None, help="Override task column name.")
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


def main() -> None:
    args = _parse_args()
    image_specs = _parse_image_specs(args.video)
    repo_id = args.repo_id or args.dataset_name
    convert_rrd_dataset_to_lerobot(
        rrd_dir=args.rrd_dir,
        output_dir=args.output,
        dataset_name=args.dataset_name,
        repo_id=repo_id,
        fps=args.fps,
        index_column=args.index,
        action_path=args.action_path,
        state_path=args.state_path,
        task_path=args.task_path,
        task_default=args.task_default,
        image_specs=image_specs,
        segments=args.segments,
        max_segments=args.max_segments,
        use_videos=not args.use_images,
        action_names=_parse_name_list(args.action_names),
        state_names=_parse_name_list(args.state_names),
        vcodec=args.vcodec,
        video_format=args.video_format,
        action_column=args.action_column,
        state_column=args.state_column,
        task_column=args.task_column,
    )


if __name__ == "__main__":
    main()
