from __future__ import annotations

from pathlib import Path

import pytest

import rerun as rr


REPO_ROOT = Path(__file__).resolve().parents[3]
URDF_PATH = REPO_ROOT / "examples" / "rust" / "animated_urdf" / "data" / "so100.urdf"


def test_urdf_tree_loading() -> None:
    tree = rr.UrdfTree.from_file_path(URDF_PATH)

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

    link_path = tree.get_link_path(child_link)
    assert link_path == "so_arm100/base/1/shoulder"
    assert tree.get_link_path_by_name("shoulder") == link_path

    root_link = tree.root_link()
    assert root_link.name == "base"
