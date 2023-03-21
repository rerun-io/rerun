#!/usr/bin/env python3
import argparse
import json
from pathlib import Path
from typing import Any, Tuple

import cv2
import numpy as np
import numpy.typing as npt
import rerun as rr
import trimesh
from download_dataset import AVAILABLE_RECORDINGS, ensure_recording_available
from scipy.spatial.transform import Rotation as R
from tqdm import tqdm

# hack for now since dataset does not provide orientation information, only known after initial visual inspection
ORIENTATION = {"48458663": "landscape", "42444949": "portrait", "41069046": "portrait"}
assert len(ORIENTATION) == len(AVAILABLE_RECORDINGS)
assert set(ORIENTATION.keys()) == set(AVAILABLE_RECORDINGS)


def load_json(js_path: Path) -> dict[str, Any]:
    with open(js_path, "r") as f:
        json_data = json.load(f)
    return dict(json_data)


def log_annotated_bboxes(annotation: dict[str, Any]) -> None:
    """Logs annotated bounding boxes to Rerun."""
    for label_info in annotation["data"]:
        object_id = label_info["objectId"]
        label = label_info["label"]
        rotation = np.array(label_info["segments"]["obbAligned"]["normalizedAxes"]).reshape(3, 3)
        transform = np.array(label_info["segments"]["obbAligned"]["centroid"]).reshape(-1, 3)[0]
        scale = np.array(label_info["segments"]["obbAligned"]["axesLengths"]).reshape(-1, 3)[0]

        rot = R.from_matrix(rotation)
        rr.log_obb(
            f"world/annotations/box-{object_id}-{label}",
            half_size=scale,
            position=transform,
            rotation_q=rot.as_quat(),
            color=[160, 230, 130, 0],
            label=label,
            timeless=True,
        )


def traj_string_to_matrix(traj_string: str) -> Tuple[str, Tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]]]:
    """
    Converts trajectory string into translation and rotation matrices.

    Args:
        traj_string: A space-delimited file where each line represents a camera position at a particular timestamp.
            The file has seven columns:
            * Column 1: timestamp
            * Columns 2-4: rotation (axis-angle representation in radians)
            * Columns 5-7: translation (usually in meters)

    Returns
    -------
        tuple: A tuple containing the timestamp (ts) and the transformation matrix (Rt).

    Raises
    ------
        AssertionError: If the input string does not contain 7 tokens.
    """
    tokens = traj_string.split()  # Split the input string into tokens
    assert len(tokens) == 7, f"Input string must have 7 tokens, but found {len(tokens)}."
    ts: str = tokens[0]  # Extract timestamp from the first token

    # Extract rotation from the second to fourth tokens
    angle_axis = [float(tokens[1]), float(tokens[2]), float(tokens[3])]
    rotation = R.from_rotvec(np.asarray(angle_axis))

    # Extract translation from the fifth to seventh tokens
    translation = np.asarray([float(tokens[4]), float(tokens[5]), float(tokens[6])])

    # Create tuple in format log_rigid3 expects
    camera_from_world = (translation, rotation.as_quat())

    return (ts, camera_from_world)


def log_arkit(recording_path: Path) -> None:
    """
    Logs ARKit recording data using Rerun.

    Args:
        recording_path (Path): The path to the ARKit recording.

    Returns
    -------
        None
    """
    video_id = recording_path.stem
    image_dir = recording_path / "lowres_wide"
    depth_dir = recording_path / "lowres_depth"
    intrinsics_dir = recording_path / "lowres_wide_intrinsics"
    traj_path = recording_path / "lowres_wide.traj"

    frame_ids = [x.name for x in sorted(depth_dir.iterdir())]
    frame_ids = [x.split(".png")[0].split("_")[1] for x in frame_ids]
    frame_ids.sort()

    # dict of timestamp to pose which is a tuple of translation and quaternion
    poses_from_traj = {}
    with open(traj_path, "r", encoding="utf-8") as f:
        traj = f.readlines()

    for line in traj:
        ts, camera_from_world = traj_string_to_matrix(line)
        # round timestamp to 3 decimal places
        ts = f"{round(float(ts), 3):.3f}"
        poses_from_traj[ts] = camera_from_world

    ply_path = recording_path / f"{recording_path.stem}_3dod_mesh.ply"
    bbox_annotations_path = recording_path / f"{recording_path.stem}_3dod_annotation.json"
    annotation = load_json(bbox_annotations_path)
    log_annotated_bboxes(annotation)

    # TODO(pablovela5620): Wait for resolution of either #1570 or #1571 for textured mesh
    # for now just use the untextered/uncolored mesh
    # # convert ply to obj
    # obj_path = recording_path / f"{recording_path.stem}_3dod_mesh.obj"
    # if not obj_path.exists():
    #     mesh = o3d.io.read_triangle_mesh(str(ply_path))
    #     o3d.io.write_triangle_mesh(str(obj_path), mesh)

    ply = trimesh.load(str(ply_path))
    verts = ply.vertices
    faces = ply.faces

    # Log the mesh
    rr.log_mesh("world/mesh_log", positions=verts, indices=faces, timeless=True)

    # with open(obj_path, mode="rb") as file:
    #     rr.log_mesh_file("world/mesh_log_file", MeshFormat.OBJ, file.read(), timeless=True)

    # To avoid logging image frames in the beginning that dont' have a trajectory
    # This causes the camera to expand in the beginning otherwise
    init_traj_found = False
    for frame_id in tqdm(frame_ids):
        rr.set_time_seconds("time", float(frame_id))
        bgr = cv2.imread(f"{image_dir}/{video_id}_{frame_id}.png")
        depth = cv2.imread(f"{depth_dir}/{video_id}_{frame_id}.png", cv2.IMREAD_ANYDEPTH)
        # Log the camera transforms:
        if str(frame_id) in poses_from_traj:
            if not init_traj_found:
                init_traj_found = True
            intrinsic_fn = intrinsics_dir / f"{video_id}_{frame_id}.pincam"
            w, h, fx, fy, cx, cy = np.loadtxt(intrinsic_fn)
            intrinsic = np.array([[fx, 0, cx], [0, fy, cy], [0, 0, 1]])
            camera_from_world = poses_from_traj[str(frame_id)]

            # TODO(pablovela5620): Fix orientation for portrait captures in 2D view once #1387 is closed.
            rr.log_rigid3(
                "world/camera",
                child_from_parent=camera_from_world,
                xyz="RDF",  # X=Right, Y=Down, Z=Forward
            )
            rr.log_pinhole("world/camera/image", child_from_parent=intrinsic, width=w, height=h)

        if not init_traj_found:
            continue

        rr.log_image("world/camera/image/rgb", bgr[..., ::-1])
        # TODO(pablovela5620): no clear way to change colormap for depth via log function
        rr.log_depth_image("world/camera/image/depth", depth, meter=1000)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    parser.add_argument(
        "--video-id",
        type=str,
        choices=AVAILABLE_RECORDINGS,
        default=AVAILABLE_RECORDINGS[0],
        help="Video ID of the ARKitScenes Dataset",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "arkitscenes")
    recording_path = dir = ensure_recording_available(args.video_id)
    log_arkit(recording_path)

    rr.script_teardown(args)
