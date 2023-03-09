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


def traj_string_to_matrix(traj_string: str) -> Tuple[str, npt.ArrayLike]:
    """
    Converts trajectory string into translation and rotation matrices.

    Args:
        traj_string (str): A space-delimited file where each line represents a camera position at a particular timestamp.
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


def get_intrinsic(intrinsics_dir: Path, frame_id: str, video_id: str) -> Tuple[npt.ArrayLike, int, int]:
    """
    Retrieves the intrinsic matrix for a given frame and video.

    Args:
        intrinsics_dir (Path): Directory containing intrinsic matrix files.
        frame_id (str): Frame identifier.
        video_id (str): Video identifier.

    Returns
    -------
        tuple: A tuple containing the intrinsic matrix, image width, and image height.

    Raises
    ------
        FileNotFoundError: If the intrinsic matrix file is not found in the directory.
    """
    intrinsic_fn: Path = (
        intrinsics_dir / f"{video_id}_{frame_id}.pincam"
    )  # Construct the filename for the intrinsic matrix

    # Check if the filename exists, if not try adjusting the frame_id by -0.001 or +0.001
    if not intrinsic_fn.exists():
        intrinsic_fn = intrinsics_dir / f"{video_id}_{float(frame_id) - 0.001:.3f}.pincam"
    if not intrinsic_fn.exists():
        intrinsic_fn = intrinsics_dir / f"{video_id}_{float(frame_id) + 0.001:.3f}.pincam"

    # Load the intrinsic matrix and extract the parameters
    w, h, fx, fy, hw, hh = np.loadtxt(intrinsic_fn)
    intrinsic = np.asarray([[fx, 0, hw], [0, fy, hh], [0, 0, 1]])

    return intrinsic, w, h


def rotate_camera_90_degrees_counterclockwise(
    translation: npt.ArrayLike, quaternion: npt.ArrayLike, intrinsics: npt.ArrayLike
) -> Tuple[npt.ArrayLike, npt.ArrayLike, npt.ArrayLike]:
    """
    Rotates the camera position by 90 degrees counterclockwise.

    Args:
        translation (ArrayLike[float]): Translation vector representing the camera position.
        quaternion (ArrayLike[float]): Quaternion representing the camera orientation.
        intrinsics (ArrayLike[float]): Intrinsic matrix representing the camera parameters.

    Returns
    -------
        tuple: A tuple containing the rotated translation, quaternion and intrinsics.

    """
    # Rotate the quaternion by 90 degrees around the z-axis
    rotation_quaternion = R.from_rotvec([0, 0, -np.pi / 2]).as_quat()
    quaternion = (R.from_quat(quaternion) * R.from_quat(rotation_quaternion)).as_quat()

    # Apply a translation in the rotated coordinate system
    translation = np.array([-translation[1], translation[0], translation[2]])

    # Apply a rotation to the intrinsics matrix
    intrinsics = np.array([[intrinsics[1, 1], 0, intrinsics[1, 2]], [0, intrinsics[0, 0], intrinsics[0, 2]], [0, 0, 1]])

    return translation, quaternion, intrinsics


def backproject(depth_image: npt.ArrayLike, intrinsics: npt.ArrayLike) -> npt.ArrayLike:
    """
    Given a depth image, generates a matching point cloud.

    Args:
        depth_image (ArrayLike[float]): 2D array representing the depth image.
        intrinsics (ArrayLike[float]): Intrinsic matrix representing the camera parameters.

    Returns
    -------
        ArrayLike[float]: A 3D array representing the point cloud.

    """
    (h, w) = depth_image.shape
    fx: float = intrinsics[0, 0]
    fy: float = intrinsics[1, 1]
    cx: float = intrinsics[0, 2]
    cy: float = intrinsics[1, 2]

    # Pre-generate image containing the x and y coordinates per pixel
    u_coords, v_coords = np.meshgrid(np.arange(0, w), np.arange(0, h))

    # Apply inverse of the intrinsics matrix:
    z = depth_image.reshape(-1)
    x = (u_coords.reshape(-1).astype(float) - cx) * z / fx
    y = (v_coords.reshape(-1).astype(float) - cy) * z / fy

    back_projected = np.vstack((x, y, z)).T
    return back_projected


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
            w, h, fx, fy, hw, hh = np.loadtxt(intrinsic_fn)
            intrinsic = np.asarray([[fx, 0, hw], [0, fy, hh], [0, 0, 1]])
            frame_pose = np.array(poses_from_traj[str(frame_id)])

            rot_matrix = frame_pose[:3, :3]
            translation = frame_pose[:3, 3]

            rot = R.from_matrix(rot_matrix[0:3, 0:3])
            quaternion = rot.as_quat()

            if orientation == "portrait":
                # TODO should probably be done via log_view_coordinates? so that rotation image also rotates the intrinsics/extrinsics
                # for some reason need to rotate the camera parameters counterclockwise, while the image is rotated clockwise
                translation, quaternion, intrinsic = rotate_camera_90_degrees_counterclockwise(
                    translation, quaternion, intrinsic
                )
                w, h = h, w
                bgr = cv2.rotate(bgr, cv2.ROTATE_90_CLOCKWISE)
                depth = cv2.rotate(depth, cv2.ROTATE_90_CLOCKWISE)

            # back project the depth map to get the 3D points, no longer needed with new backproject in GUI
            # points_3d = backproject(depth, intrinsic)

            rr.set_time_sequence("frame_id", num)
            rr.log_image("world/camera/image/rgb", bgr[..., ::-1])
            # TODO: no clear way to change colormap for dpeth via log function (only back projected points?)
            rr.log_depth_image("world/camera/image/depth", depth, meter=1000)
            # no longer needed with new backproject in GUI
            # rr.log_points("world/camera/points", positions=points_3d.reshape(-1, 3) / 1000)
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
