"""
Demonstrates how to use Rerun's chunk processing API to assemble a robot recording
from multiple file sources (MCAP, custom data, URDF, …):

- fix recording errors
- add external static data
- compute joint transforms using URDF
- insert URDF assets
- …

The resulting merged stream is saved to an RRD file, which can be
opened in the Rerun viewer or registered to a dataset catalog.
"""

from __future__ import annotations

import json
from pathlib import Path

import pyarrow as pa

import rerun as rr
from rerun.experimental import Chunk, DeriveLens, LazyChunkStream, McapReader, MutateLens, OptimizationProfile, Selector
from rerun.urdf import UrdfTree

PARENT_DIR = Path(__file__).parent
DATA_DIR = PARENT_DIR / "input_data"
OUTPUT_DIR = PARENT_DIR / "output"


def json_transforms_stream(json_path: Path) -> LazyChunkStream:
    """Loads transform data saved in JSON as a chunk stream of static Transform3D."""
    with json_path.open() as f:
        transforms = json.load(f)["transforms"]

    chunk = Chunk.from_columns(
        "/tf_static/robot_offsets",
        indexes=[],
        columns=rr.Transform3D.columns(
            translation=[transform["translation"] for transform in transforms],
            quaternion=[transform["quaternion_xyzw"] for transform in transforms],
            parent_frame=[transform["parent"] for transform in transforms],
            child_frame=[transform["child"] for transform in transforms],
        ),
    )
    return LazyChunkStream.from_iter([chunk])


def change_albedo_factor_lens(new_albedo: rr.components.AlbedoFactor) -> MutateLens:
    """Replaces Asset3D albedo factors with a fixed color."""

    return MutateLens(
        "Asset3D:albedo_factor",
        Selector(".").pipe(lambda old_albedo: pa.array([new_albedo] * len(old_albedo), type=old_albedo.type)),
    )


def joints_batch_lens(robot_urdf: UrdfTree, to_entity: str = "/tmp") -> DeriveLens:
    """Computes intermediate transform batches from each joint state message using the URDF."""
    return DeriveLens("schemas.proto.JointState:message", output_entity=to_entity).to_component(
        "rerun.urdf.JointTransformBatch",
        Selector(".").pipe(
            lambda joint_state_messages: robot_urdf.compute_joint_transform_batches(
                names=Selector(".joint_names").execute(joint_state_messages),
                values=Selector(".joint_positions").execute(joint_state_messages),
            )
        ),
    )


def output_transforms_lens() -> DeriveLens:
    """Scatters transform batches into final Transform3D rows per joint."""
    return (
        DeriveLens("rerun.urdf.JointTransformBatch", output_entity="/tf", scatter=True)
        .to_component(
            rr.Transform3D.descriptor_translation(),
            Selector(".[].translation"),
        )
        .to_component(
            rr.Transform3D.descriptor_quaternion(),
            Selector(".[].quaternion"),
        )
        .to_component(
            rr.Transform3D.descriptor_parent_frame(),
            Selector(".[].parent_frame"),
        )
        .to_component(
            rr.Transform3D.descriptor_child_frame(),
            Selector(".[].child_frame"),
        )
    )


def main() -> None:
    """Run the main chunk-processing pipeline for this example."""
    OUTPUT_DIR.mkdir(exist_ok=True)

    # Create a chunk stream from the MCAP file.
    # The reader uses Rerun's MCAP importer (like the viewer or `rerun mcap convert` CLI),
    # so we get Rerun components that we can process in-stream.
    mcap_stream = McapReader(DATA_DIR / "episode.mcap").stream()

    # The world-to-base transform offsets of the two robots are stored in a separate JSON file.
    robot_offsets_stream = json_transforms_stream(DATA_DIR / "offsets.json")

    # Load the same robot URDF twice, with distinct entity path and frame name prefixes for each robot.
    robot_urdf_left = UrdfTree.from_file_path(
        DATA_DIR / "robot.urdf",
        entity_path_prefix="robot_left",
        frame_prefix="left_",
        static_transform_entity_path="/tf_static/left_robot",
    )
    robot_urdf_right = UrdfTree.from_file_path(
        DATA_DIR / "robot.urdf",
        entity_path_prefix="robot_right",
        frame_prefix="right_",
        static_transform_entity_path="/tf_static/right_robot",
    )
    # Load the scene URDF (table & external cameras).
    scene_urdf = UrdfTree.from_file_path(DATA_DIR / "scene.urdf", static_transform_entity_path="/tf_static/scene")

    # The external camera calibration in our example MCAP has swapped width/height.
    # We can fix this with a MutateLens.
    mcap_stream = mcap_stream.lenses(
        MutateLens(
            "Pinhole:resolution",
            Selector(".").pipe(
                lambda resolution: pa.array(
                    [(height, width) for width, height in resolution.to_pylist()], type=resolution.type
                )
            ),
        ),
        content=["/external/cam_low", "/external/cam_high"],
        output_mode="forward_unmatched",
    )

    # For each robot, compute the joint transforms in batches and convert to the final Transform3D chunks.
    # We keep the original joint states in the stream ("forward_all") while dropping the temporary batch values.
    mcap_stream = (
        mcap_stream
        .lenses(joints_batch_lens(robot_urdf_left), content="/robot_left/joint_states", output_mode="forward_all")
        .lenses(output_transforms_lens(), content="/tmp", output_mode="drop_unmatched")
        .lenses(joints_batch_lens(robot_urdf_right), content="/robot_right/joint_states", output_mode="forward_all")
        .lenses(output_transforms_lens(), content="/tmp", output_mode="drop_unmatched")
    )

    # We also modify each robot's visual meshes to have custom colors / transparency by mutating the albedo factor.
    robot_urdf_left_stream = robot_urdf_left.stream().lenses(
        change_albedo_factor_lens(rr.components.AlbedoFactor([80, 120, 175, 125])),
        content="/robot_left/wxai/visual_geometries/**",
        output_mode="forward_unmatched",
    )
    robot_urdf_right_stream = robot_urdf_right.stream().lenses(
        change_albedo_factor_lens(rr.components.AlbedoFactor([200, 120, 90, 125])),
        content="/robot_right/wxai/visual_geometries/**",
        output_mode="forward_unmatched",
    )

    # Drop the collision meshes from each URDF.
    # (you can also disable them in the viewer, but here we demonstrate how to drop them entirely)
    robot_urdf_left_stream = robot_urdf_left_stream.drop(content="/robot_left/wxai/collision_geometries/**")
    robot_urdf_right_stream = robot_urdf_right_stream.drop(content="/robot_right/wxai/collision_geometries/**")

    # Merge the streams in logical groups (base recording and URDF data).
    # (alternatively we could also merge everything in one stream here, if desired)
    data_stream = LazyChunkStream.merge(
        mcap_stream,
        robot_offsets_stream,
    )
    urdf_stream = LazyChunkStream.merge(
        robot_urdf_left_stream,
        robot_urdf_right_stream,
        scene_urdf.stream(),
    )

    # Run the pipeline, materialize into a ChunkStore and optimize it before writing to an RRD.
    # Here we use an optimization profile suited for object-store (query & stream applications).
    data_stream.collect(optimize=OptimizationProfile.OBJECT_STORE).write_rrd(
        OUTPUT_DIR / "data.rrd",
        application_id="rerun_example_robot_data_preprocessing",
        recording_id="episode",
    )
    # Write also the URDF streams to an RRD.
    # Note how we use the same `recording_id` here to group the two RRD layers into the same logical recording.
    # https://rerun.io/docs/concepts/logging-and-ingestion/recordings#logical-vs-physical-recordings
    urdf_stream.collect(optimize=OptimizationProfile.OBJECT_STORE).write_rrd(
        OUTPUT_DIR / "urdf.rrd",
        application_id="rerun_example_robot_data_preprocessing",
        recording_id="episode",
    )

    print(f"\nWrote output RRDs to: {OUTPUT_DIR}")


if __name__ == "__main__":
    main()
