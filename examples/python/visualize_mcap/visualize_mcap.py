#!/usr/bin/env python3
"""Demonstrates how to load an MCAP file into the Rerun Viewer."""

from __future__ import annotations

import argparse
import xml.etree.ElementTree as ET
from pathlib import Path

import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.datatypes import ComponentSourceKind, VisualizerComponentMapping


EXAMPLE_DIR = Path(__file__).resolve().parent
MCAP_PATH = EXAMPLE_DIR.parent.parent.parent / "tests" / "assets" / "mcap" / "trossen_transfer_cube.mcap"
ASSETS_DIR = EXAMPLE_DIR / "assets"
SCENE_URDF_PATH = ASSETS_DIR / "scene.urdf"
FOLLOWER_URDF_PATH = ASSETS_DIR / "wxai_follower.urdf"


def make_urdf_with_suffix(urdf_path: Path, suffix: str) -> bytes:
    """Load a URDF file and append a suffix to all link, joint, and robot names."""
    content = urdf_path.read_text()
    tree = ET.parse(urdf_path)
    root = tree.getroot()

    # Collect all names that need suffixing
    names: set[str] = set()
    robot_name = root.attrib.get("name")
    if robot_name:
        names.add(robot_name)
    for elem in root.iter():
        if elem.tag in ("link", "joint") and "name" in elem.attrib:
            names.add(elem.attrib["name"])

    # Replace longest names first to avoid partial matches
    for name in sorted(names, key=len, reverse=True):
        content = content.replace(f'"{name}"', f'"{name}_{suffix}"')

    return content.encode()


def make_blueprint() -> rrb.Blueprint:
    return rrb.Blueprint(
        rrb.Grid(
            rrb.Horizontal(
                rrb.Vertical(
                    rrb.Spatial3DView(
                        origin="/",
                        name="3D Scene",
                        spatial_information=rrb.SpatialInformation(target_frame="world"),
                        background=rrb.Background(kind=rrb.BackgroundKind.GradientBright),
                    ),
                    rrb.Horizontal(
                        rrb.Spatial2DView(origin="/external/cam_high", name="Overhead Camera"),
                        rrb.Spatial2DView(origin="/external/cam_low", name="Low Camera"),
                    ),
                ),
                rrb.Vertical(
                    rrb.Horizontal(
                        rrb.TimeSeriesView(
                            origin="/robot_left/joint_states",
                            name="Left Joint States",
                            overrides={
                                "/robot_left/joint_states": rr.SeriesLines().visualizer(
                                    mappings=[
                                        VisualizerComponentMapping(
                                            target="SeriesLines:names",
                                            source_kind=ComponentSourceKind.SourceComponent,
                                            source_component="schemas.proto.JointState:message",
                                            selector=".joint_names[]",
                                        ),
                                        VisualizerComponentMapping(
                                            target="Scalars:scalars",
                                            source_kind=ComponentSourceKind.SourceComponent,
                                            source_component="schemas.proto.JointState:message",
                                            selector=".joint_positions[]",
                                        ),
                                    ]
                                )
                            },
                        ),
                        rrb.Spatial2DView(origin="/robot_left/wrist_camera", name="Left Wrist Camera"),
                    ),
                    rrb.Horizontal(
                        rrb.TimeSeriesView(
                            origin="/robot_right/joint_states",
                            name="Right Joint States",
                            overrides={
                                "/robot_right/joint_states": rr.SeriesLines().visualizer(
                                    mappings=[
                                        VisualizerComponentMapping(
                                            target="SeriesLines:names",
                                            source_kind=ComponentSourceKind.SourceComponent,
                                            source_component="schemas.proto.JointState:message",
                                            selector=".joint_names[]",
                                        ),
                                        VisualizerComponentMapping(
                                            target="Scalars:scalars",
                                            source_kind=ComponentSourceKind.SourceComponent,
                                            source_component="schemas.proto.JointState:message",
                                            selector=".joint_positions[]",
                                        ),
                                    ]
                                )
                            },
                        ),
                        rrb.Spatial2DView(origin="/robot_right/wrist_camera", name="Right Wrist Camera"),
                    ),
                ),
            )
        ),
    )


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Load an MCAP file into the Rerun Viewer.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_load_mcap")

    if not MCAP_PATH.exists():
        print(f"MCAP file not found: {MCAP_PATH}")
        print("Make sure git-lfs files have been pulled (`git lfs pull`).")
        raise SystemExit(1)

    rr.log_file_from_path(MCAP_PATH)
    rr.log_file_from_path(SCENE_URDF_PATH, static=True)
    for suffix in ["left", "right"]:
        urdf_contents = make_urdf_with_suffix(FOLLOWER_URDF_PATH, suffix)
        rr.log_file_from_contents(FOLLOWER_URDF_PATH, urdf_contents, static=True)
    rr.send_blueprint(make_blueprint())

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
