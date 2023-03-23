#!/usr/bin/env python3
import argparse
import json
import os
from pathlib import Path
from typing import Any, Dict, Tuple

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


def load_json(js_path: Path) -> Dict[str, Any]:
    with open(js_path, "r") as f:
        json_data = json.load(f)
    return dict(json_data)


def log_annotated_bboxes(annotation: Dict[str, Any]) -> None:
    """Logs annotated bounding boxes to Rerun."""
    # TODO(pablovela5620): Once #1581 is resolved log bounding boxes into camera view`
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
            label=label,
            timeless=True,
        )

def project_3d_bboxes_to_2d_keypoints(
    label_info: Dict[str, Any],
    camera_from_world: Tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]],
    intrinsic: npt.NDArray[np.float64],
    ) -> npt.NDArray[np.float64]:

    rotation = np.array(label_info["segments"]["obbAligned"]["normalizedAxes"]).reshape(3, 3)
    transform = np.array(label_info["segments"]["obbAligned"]["centroid"]).reshape(-1, 3)[0]
    scale = np.array(label_info["segments"]["obbAligned"]["axesLengths"]).reshape(-1, 3)[0]

    '''
    Box corner order that we return is of the format below:
      6 -------- 7
     /|         /|
    5 -------- 4 .
    | |        | |
    . 2 -------- 3
    |/         |/
    1 -------- 0
    '''
    box_corners = np.array([
        [-scale[0], -scale[1], -scale[2]],
        [-scale[0], -scale[1], scale[2]],
        [-scale[0], scale[1], -scale[2]],
        [-scale[0], scale[1], scale[2]],
        [scale[0], -scale[1], -scale[2]],
        [scale[0], -scale[1], scale[2]],
        [scale[0], scale[1], -scale[2]],
        [scale[0], scale[1], scale[2]]
    ])

    world_box_corners = (rotation @ box_corners.T).T + transform
    camera_from_world_t, camera_from_world_q = camera_from_world
    world_from_camera_t = -R.from_quat(camera_from_world_q).as_matrix() @ camera_from_world_t
    camera_box_corners = (R.from_quat(camera_from_world_q).as_matrix() @ world_box_corners.T).T + world_from_camera_t
    homogeneous_camera_box_corners = np.hstack((camera_box_corners, np.ones((8, 1))))
    image_box_corners = intrinsic @ homogeneous_camera_box_corners[:, :3].T
    image_box_corners /= image_box_corners[2, :]

    return image_box_corners


def log_camera(
    intri_path: Path,
    frame_id: str,
    poses_from_traj: Dict[str, Tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]]],
    entity_id: str,
    annotation: Dict[str, Any]
    ) -> None:
    """Logs camera intrinsics and extrinsics to Rerun."""
    w, h, fx, fy, cx, cy = np.loadtxt(intri_path)
    intrinsic = np.array([[fx, 0, cx], [0, fy, cy], [0, 0, 1]])
    camera_from_world = poses_from_traj[frame_id]

    # Log 3D bounding boxes projected into 2D image
    for label_info in annotation["data"]:
        label = label_info["label"]
        kps = project_3d_bboxes_to_2d_keypoints(label_info, camera_from_world, intrinsic)
        break

    rr.log_rigid3(
        entity_id,
        child_from_parent=camera_from_world,
        xyz="RDF",  # X=Right, Y=Down, Z=Forward
    )
    rr.log_pinhole(f"{entity_id}/image", child_from_parent=intrinsic, width=w, height=h)


def read_camera_from_world(traj_string: str) -> Tuple[str, Tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]]]:
    """
    Reads out camera_from_world transform from trajectory string.

    Args:
        traj_string: A space-delimited file where each line represents a camera position at a particular timestamp.
            The file has seven columns:
            * Column 1: timestamp
            * Columns 2-4: rotation (axis-angle representation in radians)
            * Columns 5-7: translation (usually in meters)

    Returns
    -------
    timestamp: float
        timestamp in seconds
    camera_from_world: tuple of two numpy arrays
        A tuple containing a translation vector and a quaternion that represent the camera_from_world transform

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



def find_closest_frame_id(target_id: str, frame_ids: Dict[str, Any]) -> str:
    target_value = float(target_id)
    closest_id = min(frame_ids.keys(), key=lambda x: abs(float(x) - target_value))
    return closest_id

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
    lowres_image_dir = recording_path / "lowres_wide"
    image_dir = recording_path / "wide"
    lowres_depth_dir = recording_path / "lowres_depth"
    depth_dir = recording_path / "highres_depth"
    lowres_intrinsics_dir = recording_path / "lowres_wide_intrinsics"
    intrinsics_dir = recording_path / "wide_intrinsics"
    traj_path = recording_path / "lowres_wide.traj"

    # frame_ids are indexed by timestamps, you can see more info here
    # https://github.com/apple/ARKitScenes/blob/main/threedod/README.md#data-organization-and-format-of-input-data
    depth_filenames = [x.name for x in sorted(lowres_depth_dir.iterdir())]
    lowres_frame_ids = [x.split(".png")[0].split("_")[1] for x in depth_filenames]
    lowres_frame_ids.sort()

    # dict of timestamp to pose which is a tuple of translation and quaternion
    poses_from_traj = {}
    with open(traj_path, "r", encoding="utf-8") as f:
        trajectory = f.readlines()

    for line in trajectory:
        timestamp, camera_from_world = read_camera_from_world(line)
        # round timestamp to 3 decimal places as seen in the original repo here
        # https://github.com/apple/ARKitScenes/blob/e2e975128a0a9695ea56fa215fe76b4295241538/threedod/benchmark_scripts/utils/tenFpsDataLoader.py#L247
        timestamp = f"{round(float(timestamp), 3):.3f}"
        poses_from_traj[timestamp] = camera_from_world

    rr.log_view_coordinates("world", up="+Z", right_handed=True, timeless=True)
    ply_path = recording_path / f"{recording_path.stem}_3dod_mesh.ply"
    print(f"Loading {ply_path}…")
    assert os.path.isfile(ply_path), f"Failed to find {ply_path}"

    # TODO(pablovela5620): for now just use the untextered/uncolored mesh until #1580 is resolved
    mesh_ply = trimesh.load(str(ply_path))
    rr.log_mesh(
        "world/mesh",
        positions=mesh_ply.vertices,
        indices=mesh_ply.faces,
        timeless=True
    )

    bbox_annotations_path = recording_path / f"{recording_path.stem}_3dod_annotation.json"
    annotation = load_json(bbox_annotations_path)
    log_annotated_bboxes(annotation)

    # To avoid logging image frames in the beginning that dont' have a trajectory
    # This causes the camera to expand in the beginning otherwise
    init_traj_found = False
    lowres_entity_id = "world/camera_lowres"
    highres_entity_id = "world/camera_highres"
    print("Processing frames…")
    for frame_id in tqdm(lowres_frame_ids):
        rr.set_time_seconds("time", float(frame_id))
        # load the lowres image and depth
        bgr = cv2.imread(f"{lowres_image_dir}/{video_id}_{frame_id}.png")
        depth = cv2.imread(f"{lowres_depth_dir}/{video_id}_{frame_id}.png", cv2.IMREAD_ANYDEPTH)

        high_res_exists:bool = (image_dir / f"{video_id}_{frame_id}.png").exists()

        # Log the camera transforms:
        if frame_id in poses_from_traj:
            if not init_traj_found:
                init_traj_found = True
            # only low res camera has a trajectory, high res does not
            lowres_intri_path = lowres_intrinsics_dir / f"{video_id}_{frame_id}.pincam"
            log_camera(lowres_intri_path, frame_id, poses_from_traj, lowres_entity_id, annotation)

        if not init_traj_found:
            continue

        rgb = cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB)
        rr.log_image(f"{lowres_entity_id}/image/rgb", rgb)
        # TODO(pablovela5620): no clear way to change colormap for depth via log function
        rr.log_depth_image(f"{lowres_entity_id}/image/depth", depth, meter=1000)

        if high_res_exists:
            rr.set_time_seconds("time high resolution", float(frame_id))
            closest_lowres_frame_id = find_closest_frame_id(frame_id, poses_from_traj)
            highres_intri_path = intrinsics_dir / f"{video_id}_{frame_id}.pincam"
            log_camera(highres_intri_path, closest_lowres_frame_id, poses_from_traj, highres_entity_id, annotation)

            # load the highres image and depth if they exist
            highres_bgr = cv2.imread(f"{image_dir}/{video_id}_{frame_id}.png")
            highres_depth = cv2.imread(f"{depth_dir}/{video_id}_{frame_id}.png", cv2.IMREAD_ANYDEPTH)

            highres_rgb = cv2.cvtColor(highres_bgr, cv2.COLOR_BGR2RGB)
            rr.log_image(f"{highres_entity_id}/image/rgb", highres_rgb)
            rr.log_depth_image(f"{highres_entity_id}/image/depth", highres_depth, meter=1000)


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
