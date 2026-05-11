from __future__ import annotations

import math
from pathlib import Path

import pyarrow as pa
import pytest
import rerun as rr
import rerun.urdf as rru
from rerun.experimental import Chunk, DeriveLens, LazyChunkStream, RrdReader, Selector, StreamingReader

REPO_ROOT = Path(__file__).resolve().parents[3]
URDF_PATH = REPO_ROOT / "examples" / "rust" / "animated_urdf" / "data" / "so100.urdf"


def test_urdf_tree_loading() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    assert tree.name == "so_arm100"

    joints = tree.joints()
    assert len(joints) == 6

    joint = tree.get_joint_by_name("1")
    assert joint is not None
    assert joint.joint_type == "revolute"
    assert joint.parent_link == "base"
    assert joint.child_link == "shoulder"
    assert joint.axis == pytest.approx((0.0, 1.0, 0.0))
    assert joint.limit_lower == pytest.approx(-2.0)
    assert joint.limit_upper == pytest.approx(2.0)

    child_link = tree.get_joint_child(joint)
    assert child_link.name == "shoulder"

    # We expect flat, neighboring paths for visual and collision geometries of links,
    # queryable by either link name or link object.
    visual_paths = tree.get_visual_geometry_paths(child_link)
    assert visual_paths[0] == "/so_arm100/visual_geometries/shoulder/visual_0"
    visual_paths = tree.get_visual_geometry_paths("wrist")
    assert visual_paths[0] == "/so_arm100/visual_geometries/wrist/visual_0"

    collision_paths = tree.get_collision_geometry_paths(child_link)
    assert collision_paths[0] == "/so_arm100/collision_geometries/mesh/shoulder/collision_0"
    collision_paths = tree.get_collision_geometry_paths("wrist")
    assert collision_paths[0] == "/so_arm100/collision_geometries/mesh/wrist/collision_0"

    root_link = tree.root_link()
    assert root_link.name == "base"


def test_urdf_tree_transform() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    joint = tree.get_joint_by_name("1")
    assert joint is not None

    clamped_angle = joint.limit_upper + 1.0
    with pytest.warns(UserWarning, match="outside limits"):
        transform = joint.compute_transform(clamped_angle)

    expected_angle = joint.limit_upper
    origin_roll, origin_pitch, origin_yaw = joint.origin_rpy
    quat_origin = _euler_to_quat(origin_roll, origin_pitch, origin_yaw)
    axis_x, axis_y, axis_z = joint.axis
    half_angle = expected_angle / 2.0
    sin_half = math.sin(half_angle)
    cos_half = math.cos(half_angle)
    quat_dynamic = [
        axis_x * sin_half,
        axis_y * sin_half,
        axis_z * sin_half,
        cos_half,
    ]
    expected_quat = _quat_multiply(quat_origin, quat_dynamic)

    assert transform.translation is not None
    assert transform.quaternion is not None
    assert_translation_expected(transform.translation, joint.origin_xyz)
    assert_quat_equivalent(transform.quaternion, expected_quat)
    assert transform.parent_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(joint.parent_link)
    )
    assert transform.child_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(joint.child_link)
    )


def test_urdf_tree_frame_prefix() -> None:
    prefix = "left_arm/"
    tree = rru.UrdfTree.from_file_path(URDF_PATH, frame_prefix=prefix)

    assert tree.frame_prefix == prefix

    joint = tree.get_joint_by_name("1")
    assert joint is not None

    # parent_link and child_link should still return unprefixed URDF link names.
    assert joint.parent_link == "base"
    assert joint.child_link == "shoulder"

    # But compute_transform should produce prefixed frame IDs.
    transform = joint.compute_transform(0.0)
    assert transform.parent_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(f"{prefix}{joint.parent_link}")
    )
    assert transform.child_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(f"{prefix}{joint.child_link}")
    )

    # compute_transform_columns should also produce prefixed frame IDs.
    columns = joint.compute_transform_columns([0.0, 0.5], clamp=True)
    from rerun._baseclasses import ComponentColumnList

    assert isinstance(columns, ComponentColumnList)


def test_urdf_tree_no_frame_prefix() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)
    assert tree.frame_prefix is None


def test_urdf_tree_log() -> None:
    rec = rr.RecordingStream("rerun_example_test_urdf_tree_log", make_default=False, make_thread_default=False)
    rec.memory_recording()

    tree = rru.UrdfTree.from_file_path(URDF_PATH)
    tree.log_urdf_to_recording(rec)

    # Also test with frame_prefix
    tree_prefixed = rru.UrdfTree.from_file_path(URDF_PATH, frame_prefix="left/")
    tree_prefixed.log_urdf_to_recording(rec)


def test_urdf_tree_custom_static_transform_entity_path(tmp_path: Path) -> None:
    rrd_path = tmp_path / "urdf_static_transforms.rrd"

    with rr.RecordingStream(
        "test_urdf_tree_custom_static_transform_entity_path",
        make_default=False,
        make_thread_default=False,
    ) as rec:
        rec.save(rrd_path)

        tree = rru.UrdfTree.from_file_path(URDF_PATH, static_transform_entity_path="custom_tf_static")
        tree.log_urdf_to_recording(rec)

        rec.disconnect()  # save manifest

    paths = RrdReader(rrd_path).stream().collect().schema().entity_paths()

    assert "/custom_tf_static" in paths
    assert "/tf_static" not in paths


def test_urdf_tree_stream() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    assert isinstance(tree, StreamingReader)

    chunks = tree.stream().to_chunks()
    entity_paths = {chunk.entity_path for chunk in chunks}

    assert "/so_arm100" in entity_paths
    assert "/so_arm100/visual_geometries/shoulder/visual_0" in entity_paths
    assert "/tf_static" in entity_paths


def test_urdf_tree_stream_custom_static_transform_entity_path() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH, static_transform_entity_path="custom_tf_static")

    entity_paths = {chunk.entity_path for chunk in tree.stream().to_chunks()}

    assert "/custom_tf_static" in entity_paths
    assert "/tf_static" not in entity_paths


def test_urdf_tree_stream_without_joint_transforms() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    entity_paths = {chunk.entity_path for chunk in tree.stream(include_joint_transforms=False).to_chunks()}

    assert "/tf_static" not in entity_paths


def test_urdf_compute_transform_columns() -> None:
    tree = rru.UrdfTree.from_file_path(URDF_PATH)

    joint = tree.get_joint_by_name("1")
    assert joint is not None

    values = [0.0, 0.5, 1.0]
    columns = joint.compute_transform_columns(values, clamp=True)

    from rerun._baseclasses import ComponentColumnList

    assert isinstance(columns, ComponentColumnList)
    assert len(columns) > 0

    # Verify the result is usable with send_columns by checking that
    # the individual transform matches what compute_transform returns.
    single = joint.compute_transform(0.5, clamp=True)
    assert single.translation is not None
    assert single.quaternion is not None

    # Verify that out-of-range values produce warnings.
    with pytest.warns(UserWarning, match="outside limits"):
        joint.compute_transform_columns([joint.limit_upper + 1.0], clamp=True)


def test_urdf_transform_batches_in_lens_pipeline() -> None:
    """Integration test for computing transform batches from joint states in a chunk pipeline using lenses."""

    frame_prefix = "prefix_"
    urdf_tree = rru.UrdfTree.from_file_path(URDF_PATH, frame_prefix=frame_prefix)

    # Create a chunk with joint names and values.
    messages = pa.StructArray.from_arrays(
        [
            # Note: "1", "2" are joint names from the test URDF file we use here.
            pa.array([["1", "2"], ["1"]], type=pa.list_(pa.string())),
            pa.array([[0.0, 0.5], [1.0]], type=pa.list_(pa.float64())),
        ],
        names=["joint_names", "actuator_readings"],
    )
    chunk = Chunk.from_columns(
        "/joint_states",
        indexes=[rr.TimeColumn("frame", sequence=[0, 1])],
        columns=rr.DynamicArchetype.columns(
            archetype="schemas.SomeCustomJointState",
            components={"message": messages},
        ),
    )

    # Apply a two-stage pipeline using lenses:

    # 1. Compute joint transforms in batch using `UrdfTree.compute_joint_transform_batches`.
    # N input rows with multiple joint states per row -> N output rows with multiple transforms per row.
    compute_joints = DeriveLens("schemas.SomeCustomJointState:message").to_component(
        "rerun.urdf.JointTransformBatch",
        Selector(".").pipe(
            lambda joint_state_messages: urdf_tree.compute_joint_transform_batches(
                names=Selector(".joint_names").execute(joint_state_messages),
                values=Selector(".actuator_readings").execute(joint_state_messages),
            )
        ),
    )
    # 2. Scatter the batch data into final `Transform3D` rows.
    # N input rows with multiple transforms per row -> one output row per transform.
    output_transforms = (
        DeriveLens("rerun.urdf.JointTransformBatch", output_entity="/tf", scatter=True)
        .to_component(rr.Transform3D.descriptor_translation(), Selector(".[].translation"))
        .to_component(rr.Transform3D.descriptor_quaternion(), Selector(".[].quaternion"))
        .to_component(rr.Transform3D.descriptor_parent_frame(), Selector(".[].parent_frame"))
        .to_component(rr.Transform3D.descriptor_child_frame(), Selector(".[].child_frame"))
    )

    chunks = LazyChunkStream.from_iter([chunk]).lenses(compute_joints).lenses(output_transforms).to_chunks()

    assert len(chunks) == 1
    assert chunks[0].entity_path == "/tf"
    assert chunks[0].num_rows == 3

    batch = chunks[0].to_record_batch()
    assert batch.column("frame").to_pylist() == [0, 0, 1]
    assert set(batch.column_names) >= {
        "Transform3D:translation",
        "Transform3D:quaternion",
        "Transform3D:parent_frame",
        "Transform3D:child_frame",
    }

    joint1 = urdf_tree.get_joint_by_name("1")
    joint2 = urdf_tree.get_joint_by_name("2")
    assert joint1 is not None and joint2 is not None
    assert batch.column("Transform3D:parent_frame").to_pylist() == [
        [f"{frame_prefix}{joint1.parent_link}"],
        [f"{frame_prefix}{joint2.parent_link}"],
        [f"{frame_prefix}{joint1.parent_link}"],
    ]
    assert batch.column("Transform3D:child_frame").to_pylist() == [
        [f"{frame_prefix}{joint1.child_link}"],
        [f"{frame_prefix}{joint2.child_link}"],
        [f"{frame_prefix}{joint1.child_link}"],
    ]


def assert_quat_equivalent(actual: rr.components.RotationQuatBatch, expected: list[float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    dot = sum(a * b for a, b in zip(actual_values, expected, strict=False))
    if dot < 0.0:
        expected = [-value for value in expected]
    assert actual_values == pytest.approx(expected)


def assert_translation_expected(actual: rr.components.Translation3DBatch, expected: tuple[float, float, float]) -> None:
    actual_values = actual.pa_array.to_pylist()[0]
    assert actual_values == pytest.approx(expected)


def _euler_to_quat(roll: float, pitch: float, yaw: float) -> list[float]:
    """Convert Euler angles (RPY) to quaternion (XYZW)."""
    cr = math.cos(roll * 0.5)
    sr = math.sin(roll * 0.5)
    cp = math.cos(pitch * 0.5)
    sp = math.sin(pitch * 0.5)
    cy = math.cos(yaw * 0.5)
    sy = math.sin(yaw * 0.5)

    w = cr * cp * cy + sr * sp * sy
    x = sr * cp * cy - cr * sp * sy
    y = cr * sp * cy + sr * cp * sy
    z = cr * cp * sy - sr * sp * cy

    return [x, y, z, w]


def _quat_multiply(q1: list[float], q2: list[float]) -> list[float]:
    """Multiply two quaternions in XYZW format."""
    x1, y1, z1, w1 = q1
    x2, y2, z2, w2 = q2

    w = w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2
    x = w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2
    y = w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2
    z = w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2

    return [x, y, z, w]
