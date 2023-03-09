#!/usr/bin/env python3
import argparse
from pathlib import Path
from typing import Tuple

import cv2
import numpy as np
import numpy.typing as npt
import rerun as rr
from download_dataset import AVAILABLE_RECORDINGS, ensure_recording_available
from scipy.spatial.transform import Rotation as R
from tqdm import tqdm

# hack for now since dataset does not provide orientation information, only known after initial visual inspection
ORIENTATION = {"42444949": "portrait", "48458663": "landscape", "41069046": "portrait"}
assert len(ORIENTATION) == len(AVAILABLE_RECORDINGS)
assert set(ORIENTATION.keys()) == set(AVAILABLE_RECORDINGS)


def traj_string_to_matrix(traj_string: str) -> Tuple[str, npt.NDArray[np.float64]]:
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
    r_w_to_p = cv2.Rodrigues(np.asarray(angle_axis))[0]

    # Extract translation from the fifth to seventh tokens
    t_w_to_p = np.asarray([float(tokens[4]), float(tokens[5]), float(tokens[6])])

    # Construct the extrinsics matrix
    extrinsics = np.eye(4, 4)
    extrinsics[:3, :3] = r_w_to_p
    extrinsics[:3, -1] = t_w_to_p

    # Compute the inverse of extrinsics matrix to get the rotation and translation matrices
    Rt = np.linalg.inv(extrinsics)

    return (ts, Rt)


def rotate_camera_90_degrees_counterclockwise(
    translation: npt.NDArray[np.float64], quaternion: npt.NDArray[np.float64], intrinsics: npt.NDArray[np.float64]
) -> Tuple[npt.NDArray[np.float64], npt.NDArray[np.float64], npt.NDArray[np.float64]]:
    """
    Rotates the camera position by 90 degrees counterclockwise.

    Args:
        translation: Translation vector representing the camera position.
        quaternion: Quaternion representing the camera orientation.
        intrinsics: Intrinsic matrix representing the camera parameters.

    Returns
    -------
        tuple: A tuple containing the rotated translation, quaternion and intrinsics.

    """
    # Rotate the quaternion by 90 degrees around the z-axis
    rotation_quaternion = R.from_rotvec([0, 0, -np.pi / 2]).as_quat()
    quaternion = (R.from_quat(quaternion) * R.from_quat(rotation_quaternion)).as_quat()

    # Apply a translation in the rotated coordinate system
    translation = np.array([-1, 1, 1]) * np.array(translation)[[1, 0, 2]]

    # Apply a rotation to the intrinsics matrix
    swizzle_x_y = np.array([[0, 1, 0], [1, 0, 0], [0, 0, 1]])
    intrinsics = swizzle_x_y @ intrinsics @ swizzle_x_y

    return translation, quaternion, intrinsics


def log_arkit(recording_path: Path, orientation: str) -> None:
    """
    Logs ARKit recording data using Rerun.

    Args:
        recording_path (Path): The path to the ARKit recording.
        orientation (str): The orientation of the recording, either "landscape" or "portrait".

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

    poses_from_traj = {}
    with open(traj_path, "r", encoding="utf-8") as f:
        traj = f.readlines()

    for line in traj:
        ts, Rt = traj_string_to_matrix(line)
        # round timestamp to 3 decimal places
        ts = f"{round(float(ts), 3):.3f}"
        poses_from_traj[ts] = Rt

    for num, frame_id in enumerate(tqdm(frame_ids)):
        bgr = cv2.imread(f"{image_dir}/{video_id}_{frame_id}.png")
        depth = cv2.imread(f"{depth_dir}/{video_id}_{frame_id}.png", cv2.IMREAD_ANYDEPTH)
        # Log the camera transforms:
        if str(frame_id) in poses_from_traj:
            intrinsic_fn = intrinsics_dir / f"{video_id}_{frame_id}.pincam"
            w, h, fx, fy, cx, cy = np.loadtxt(intrinsic_fn)
            intrinsic = np.array([[fx, 0, cx], [0, fy, cy], [0, 0, 1]])
            frame_pose = np.array(poses_from_traj[str(frame_id)])

            rot_matrix = frame_pose[:3, :3]
            translation = frame_pose[:3, 3]

            rot = R.from_matrix(rot_matrix[0:3, 0:3])
            quaternion = rot.as_quat()

            if orientation == "portrait":
                # TODO(pablovela5620) should probably be done via log_view_coordinates?
                # so that rotation image also rotates the intrinsics/extrinsics
                translation, quaternion, intrinsic = rotate_camera_90_degrees_counterclockwise(
                    translation, quaternion, intrinsic
                )
                w, h = h, w
                bgr = cv2.rotate(bgr, cv2.ROTATE_90_CLOCKWISE)
                depth = cv2.rotate(depth, cv2.ROTATE_90_CLOCKWISE)

            rr.set_time_sequence("frame_id", num)
            rr.log_image("world/camera/image/rgb", bgr[..., ::-1])
            # TODO(pablovela5620): no clear way to change colormap for depth via log function
            # (only back projected points?)
            rr.log_depth_image("world/camera/image/depth", depth, meter=1000)
            rr.log_rigid3(
                "world/camera",
                parent_from_child=(translation, quaternion),
                xyz="RDF",  # X=Right, Y=Down, Z=Forward
            )
            rr.log_pinhole("world/camera/image", child_from_parent=intrinsic, width=w, height=h)


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
    orientation = ORIENTATION[args.video_id]
    log_arkit(recording_path, orientation)

    rr.script_teardown(args)
