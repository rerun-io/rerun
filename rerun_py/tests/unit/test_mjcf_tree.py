from __future__ import annotations

import math
from pathlib import Path

import pyarrow as pa
import pytest
import rerun as rr
import rerun.mjcf as rrm
from rerun._baseclasses import ComponentColumnList
from rerun.chunk import Chunk, DeriveLens, LazyChunkStream, RrdReader, Selector, StreamingReader

from .kinematics_helpers import assert_quat_equivalent, assert_translation_expected

REPO_ROOT = Path(__file__).resolve().parents[3]
MJCF_PATH = REPO_ROOT / "crates" / "data_flow" / "re_importer" / "tests" / "assets" / "mjcf" / "arm.xml"


def test_mjcf_tree_loading() -> None:
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH)

    assert tree.name == "test_arm"
    assert tree.root_body().name == "world"
    assert [body.name for body in tree.bodies()] == ["world", "base", "shoulder", "slider", "wrist"]
    assert [joint.name for joint in tree.joints()] == ["yaw", "pitch", "extend", "limited", "ball"]

    pitch = tree.get_joint_by_name("pitch")
    assert pitch is not None
    assert pitch.joint_type == "hinge"
    assert pitch.body == "shoulder"
    assert pitch.parent_body == "base"
    assert pitch.axis == pytest.approx((0.0, 1.0, 0.0))
    assert pitch.anchor == pytest.approx((1.0, 0.0, 0.0))
    assert pitch.range is None

    limited = tree.get_joint_by_name("limited")
    assert limited is not None
    assert limited.range == pytest.approx((-1.0, 1.0))

    extend = tree.get_joint_by_name("extend")
    assert extend is not None
    assert extend.joint_type == "slide"
    assert extend.reference == pytest.approx(0.5)

    shoulder = tree.get_joint_body(pitch)
    assert shoulder.name == "shoulder"
    assert shoulder.parent == "base"
    assert [joint.name for joint in shoulder.joints()] == ["yaw", "pitch"]
    assert shoulder.translation == pytest.approx((0.0, 0.0, 1.0))
    assert shoulder.quaternion_xyzw == pytest.approx((0.0, 0.0, 0.0, 1.0))

    assert tree.get_body_by_name("nope") is None
    assert tree.get_joint_by_name("nope") is None

    assert tree.get_visual_geometry_paths(shoulder) == ["/test_arm/visual_geometries/shoulder/shoulder_visual"]
    assert tree.get_collision_geometry_paths("shoulder") == [
        "/test_arm/collision_geometries/capsule/shoulder/shoulder_collision"
    ]
    assert tree.get_collision_geometry_paths("nope") == []


def test_mjcf_joint_transform() -> None:
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH)

    yaw = tree.get_joint_by_name("yaw")
    assert yaw is not None
    transform = yaw.compute_transform(math.pi / 2)

    assert transform.translation is not None
    assert transform.quaternion is not None
    assert_translation_expected(transform.translation, (0.0, 0.0, 1.0))
    assert_quat_equivalent(transform.quaternion, [0.0, 0.0, math.sin(math.pi / 4), math.cos(math.pi / 4)])
    assert transform.parent_frame == rr.components.TransformFrameIdBatch(rr.components.TransformFrameId("base"))
    assert transform.child_frame == rr.components.TransformFrameIdBatch(rr.components.TransformFrameId("shoulder::yaw"))

    # The second joint of a body moves it relative to the frame after the first, and swings around its anchor.
    pitch = tree.get_joint_by_name("pitch")
    assert pitch is not None
    transform = pitch.compute_transform(math.pi / 2)
    assert transform.translation is not None
    assert_translation_expected(transform.translation, (1.0, 0.0, 1.0))
    assert transform.parent_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId("shoulder::yaw")
    )
    assert transform.child_frame == rr.components.TransformFrameIdBatch(rr.components.TransformFrameId("shoulder"))

    # Slide values are relative to the joint's `ref`, like MuJoCo's `qpos`.
    extend = tree.get_joint_by_name("extend")
    assert extend is not None
    transform = extend.compute_transform(1.5)
    assert transform.translation is not None
    assert_translation_expected(transform.translation, (2.0, 0.0, 0.0))


def test_mjcf_joint_transform_clamping() -> None:
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH)

    limited = tree.get_joint_by_name("limited")
    assert limited is not None

    with pytest.warns(UserWarning, match="outside limits"):
        transform = limited.compute_transform(2.0)
    assert transform.quaternion is not None
    assert_quat_equivalent(transform.quaternion, [math.sin(0.5), 0.0, 0.0, math.cos(0.5)])

    transform = limited.compute_transform(2.0, clamp=False)
    assert transform.quaternion is not None
    assert_quat_equivalent(transform.quaternion, [math.sin(1.0), 0.0, 0.0, math.cos(1.0)])


def test_mjcf_unsupported_joint_type() -> None:
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH)

    ball = tree.get_joint_by_name("ball")
    assert ball is not None
    with pytest.raises(NotImplementedError):
        ball.compute_transform(0.0)

    with pytest.raises(NotImplementedError):
        tree.compute_joint_transform_batches(
            pa.array([["ball"]], type=pa.list_(pa.string())),
            pa.array([[0.0]], type=pa.list_(pa.float64())),
        )

    with pytest.raises(ValueError, match="does not contain joint"):
        tree.compute_joint_transform_batches(
            pa.array([["nope"]], type=pa.list_(pa.string())),
            pa.array([[0.0]], type=pa.list_(pa.float64())),
        )


def test_mjcf_compute_transform_columns() -> None:
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH)

    limited = tree.get_joint_by_name("limited")
    assert limited is not None

    columns = limited.compute_transform_columns([0.0, 0.5, 1.0])
    assert isinstance(columns, ComponentColumnList)
    assert len(columns) > 0

    with pytest.warns(UserWarning, match="outside limits"):
        limited.compute_transform_columns([2.0])


def test_mjcf_tree_frame_prefix() -> None:
    prefix = "left_arm/"
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH, frame_prefix=prefix)
    assert tree.frame_prefix == prefix
    assert rrm.MjcfTree.from_file_path(MJCF_PATH).frame_prefix is None

    yaw = tree.get_joint_by_name("yaw")
    assert yaw is not None

    # Body names stay unprefixed, but the frames of computed transforms are prefixed.
    assert yaw.body == "shoulder"
    transform = yaw.compute_transform(0.0)
    assert transform.parent_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(f"{prefix}base")
    )
    assert transform.child_frame == rr.components.TransformFrameIdBatch(
        rr.components.TransformFrameId(f"{prefix}shoulder::yaw")
    )


@pytest.mark.parametrize(
    ("prefix", "expected_prefix"),
    [
        ("world/robot", "/world/robot"),
        ("/world/robot/", "/world/robot"),
        ("robot", "/robot"),
        ("", ""),
        (r"world\/robot", r"/world\/robot"),
    ],
)
def test_mjcf_tree_entity_path_prefix(prefix: str, expected_prefix: str) -> None:
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH, entity_path_prefix=prefix)
    expected_root = f"{expected_prefix}/test_arm"
    expected_visual = f"{expected_root}/visual_geometries/shoulder/shoulder_visual"
    expected_collision = f"{expected_root}/collision_geometries/capsule/shoulder/shoulder_collision"

    assert expected_visual in tree.get_visual_geometry_paths("shoulder")
    assert expected_collision in tree.get_collision_geometry_paths("shoulder")
    entity_paths = {chunk.entity_path for chunk in tree.stream().to_chunks()}
    assert {expected_root, expected_visual, expected_collision, "/tf_static"} <= entity_paths


def test_mjcf_tree_prefix_matches_log_file_from_path(tmp_path: Path) -> None:
    prefix = "world/robot"
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH, entity_path_prefix=prefix)
    tree_paths = {chunk.entity_path for chunk in tree.stream().to_chunks()}
    rrd_path = tmp_path / "mjcf.rrd"

    with rr.RecordingStream("rerun_example_mjcf_prefix_test", make_default=False, make_thread_default=False) as rec:
        rec.save(rrd_path)
        rr.log_file_from_path(MJCF_PATH, entity_path_prefix=prefix, static=True, recording=rec)

    logged_paths = set(RrdReader(rrd_path).stream().collect().schema().entity_paths())
    assert tree_paths == logged_paths


def test_mjcf_tree_log(tmp_path: Path) -> None:
    rrd_path = tmp_path / "mjcf.rrd"

    with rr.RecordingStream("rerun_example_test_mjcf_tree_log", make_default=False, make_thread_default=False) as rec:
        rec.save(rrd_path)
        tree = rrm.MjcfTree.from_file_path(MJCF_PATH, static_transform_entity_path="custom_tf_static")
        tree.log_to_recording(rec)

    paths = RrdReader(rrd_path).stream().collect().schema().entity_paths()
    assert "/test_arm/visual_geometries/shoulder/shoulder_visual" in paths
    assert "/custom_tf_static" in paths
    assert "/tf_static" not in paths


def test_mjcf_tree_stream() -> None:
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH)
    assert isinstance(tree, StreamingReader)

    entity_paths = {chunk.entity_path for chunk in tree.stream().to_chunks()}
    assert "/test_arm" in entity_paths
    assert "/test_arm/visual_geometries/base/base_visual" in entity_paths
    assert "/tf_static" in entity_paths

    entity_paths = {chunk.entity_path for chunk in tree.stream(include_joint_transforms=False).to_chunks()}
    assert "/tf_static" not in entity_paths


def test_mjcf_transform_batches_in_lens_pipeline() -> None:
    """Every joint gets its own transform, also when several joints move one body."""

    tree = rrm.MjcfTree.from_file_path(MJCF_PATH, frame_prefix="prefix_")

    messages = pa.StructArray.from_arrays(
        [
            pa.array([["yaw", "pitch", "extend"], ["limited"]], type=pa.list_(pa.string())),
            pa.array([[0.0, 0.5, 1.0], [0.25]], type=pa.list_(pa.float64())),
        ],
        names=["joint_names", "qpos"],
    )
    chunk = Chunk.from_columns(
        "/joint_states",
        indexes=[rr.TimeColumn("frame", sequence=[0, 1])],
        columns=rr.DynamicArchetype.columns(
            archetype="schemas.SomeCustomJointState",
            components={"message": messages},
        ),
    )

    compute_joints = DeriveLens("schemas.SomeCustomJointState:message").to_component(
        "rerun.mjcf.JointTransformBatch",
        Selector(".").pipe(
            lambda joint_state_messages: tree.compute_joint_transform_batches(
                names=Selector(".joint_names").execute(joint_state_messages),
                values=Selector(".qpos").execute(joint_state_messages),
            )
        ),
    )
    output_transforms = (
        DeriveLens("rerun.mjcf.JointTransformBatch", output_entity="/tf", scatter=True)
        .to_component(rr.Transform3D.descriptor_translation(), Selector(".[].translation"))
        .to_component(rr.Transform3D.descriptor_quaternion(), Selector(".[].quaternion"))
        .to_component(rr.Transform3D.descriptor_parent_frame(), Selector(".[].parent_frame"))
        .to_component(rr.Transform3D.descriptor_child_frame(), Selector(".[].child_frame"))
    )

    chunks = LazyChunkStream.from_iter([chunk]).lenses(compute_joints).lenses(output_transforms).to_chunks()

    assert len(chunks) == 1
    batch = chunks[0].to_record_batch()
    assert batch.column("frame").to_pylist() == [0, 0, 0, 1]
    assert batch.column("Transform3D:child_frame").to_pylist() == [
        ["prefix_shoulder::yaw"],
        ["prefix_shoulder"],
        ["prefix_slider"],
        ["prefix_wrist::limited"],
    ]
    assert batch.column("Transform3D:parent_frame").to_pylist() == [
        ["prefix_base"],
        ["prefix_shoulder::yaw"],
        ["prefix_shoulder"],
        ["prefix_shoulder"],
    ]


def test_objects_from_another_tree(tmp_path: Path) -> None:
    other_path = tmp_path / "other.xml"
    other_path.write_text('<mujoco><worldbody><body name="lone"><joint name="only"/></body></worldbody></mujoco>')
    other = rrm.MjcfTree.from_file_path(other_path)
    tree = rrm.MjcfTree.from_file_path(MJCF_PATH)

    lone = other.get_body_by_name("lone")
    assert lone is not None
    assert tree.get_visual_geometry_paths(lone) == []
    assert tree.get_collision_geometry_paths(lone) == []

    only = other.get_joint_by_name("only")
    assert only is not None
    with pytest.raises(ValueError, match="not part of"):
        tree.get_joint_body(only)


def test_mjcf_body_pose(tmp_path: Path) -> None:
    """MJCF writes quaternions as `wxyz`, while the body exposes them as `xyzw`."""
    path = tmp_path / "rotated.xml"
    path.write_text('<mujoco><worldbody><body name="b" pos="1 2 3" quat="0 0 0 1"/></worldbody></mujoco>')
    body = rrm.MjcfTree.from_file_path(path).get_body_by_name("b")
    assert body is not None
    assert body.translation == pytest.approx((1.0, 2.0, 3.0))
    assert body.quaternion_xyzw == pytest.approx((0.0, 0.0, 1.0, 0.0))


@pytest.mark.parametrize(
    ("contents", "prefix"),
    [(None, "Failed to read MJCF file: "), ("<mujoco><worldbody>", "Failed to parse MJCF file: ")],
)
def test_mjcf_load_error(tmp_path: Path, contents: str | None, prefix: str) -> None:
    """The error says once what failed, and ends with the file path."""
    path = tmp_path / "broken.xml"
    if contents is not None:
        path.write_text(contents)
    with pytest.raises(RuntimeError) as exc_info:
        rrm.MjcfTree.from_file_path(path)
    message = str(exc_info.value)
    assert message.startswith(prefix)
    assert message.count("MJCF file") == 1
    assert message.endswith(f"\nFile path: {path}")
