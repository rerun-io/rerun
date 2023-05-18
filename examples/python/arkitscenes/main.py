#!/usr/bin/env python3
import argparse
import json
import os
from pathlib import Path, PosixPath
from typing import Any, Dict, List, Tuple

import cv2
import matplotlib.pyplot as plt
import numpy as np
import numpy.typing as npt
import depthai_viewer as viewer
import trimesh
from download_dataset import AVAILABLE_RECORDINGS, ensure_recording_available
from scipy.spatial.transform import Rotation as R
from tqdm import tqdm

Color = Tuple[float, float, float, float]

# hack for now since dataset does not provide orientation information, only known after initial visual inspection
ORIENTATION = {
    "48458663": "landscape",
    "42444949": "portrait",
    "41069046": "portrait",
    "41125722": "portrait",
    "41125763": "portrait",
    "42446167": "portrait",
}
assert len(ORIENTATION) == len(AVAILABLE_RECORDINGS)
assert set(ORIENTATION.keys()) == set(AVAILABLE_RECORDINGS)


def load_json(js_path: Path) -> Dict[str, Any]:
    with open(js_path, "r") as f:
        json_data = json.load(f)  # type: Dict[str, Any]
    return json_data


def log_annotated_bboxes(annotation: Dict[str, Any]) -> Tuple[npt.NDArray[np.float64], List[str], List[Color]]:
    """
    Logs annotated oriented bounding boxes to Rerun.

    We currently calculate and return the 3D bounding boxes keypoints, labels, and colors for each object to log them in
    each camera frame TODO(pablovela5620): Once #1581 is resolved this can be removed.

    annotation json file
    |  |-- label: object name of bounding box
    |  |-- axesLengths[x, y, z]: size of the origin bounding-box before transforming
    |  |-- centroid[]: the translation matrix (1,3) of bounding-box
    |  |-- normalizedAxes[]: the rotation matrix (3,3) of bounding-box
    """
    bbox_list = []
    bbox_labels = []
    num_objects = len(annotation["data"])
    # Generate a color per object that can be reused across both 3D obb and their 2D projections
    # TODO(pablovela5620): Once #1581 or #1728 is resolved this can be removed
    color_positions = np.linspace(0, 1, num_objects)
    colormap = plt.cm.get_cmap("viridis")
    colors = [colormap(pos) for pos in color_positions]

    for i, label_info in enumerate(annotation["data"]):
        uid = label_info["uid"]
        label = label_info["label"]

        half_size = 0.5 * np.array(label_info["segments"]["obbAligned"]["axesLengths"]).reshape(-1, 3)[0]
        centroid = np.array(label_info["segments"]["obbAligned"]["centroid"]).reshape(-1, 3)[0]
        rotation = np.array(label_info["segments"]["obbAligned"]["normalizedAxes"]).reshape(3, 3)

        rot = R.from_matrix(rotation).inv()

        viewer.log_obb(
            f"world/annotations/box-{uid}-{label}",
            half_size=half_size,
            position=centroid,
            rotation_q=rot.as_quat(),
            label=label,
            color=colors[i],
            timeless=True,
        )

        box3d = compute_box_3d(half_size, centroid, rotation)
        bbox_list.append(box3d)
        bbox_labels.append(label)
    bboxes_3d = np.array(bbox_list)
    return bboxes_3d, bbox_labels, colors


def compute_box_3d(
    half_size: npt.NDArray[np.float64], transform: npt.NDArray[np.float64], rotation: npt.NDArray[np.float64]
) -> npt.NDArray[np.float64]:
    """
    Given obb compute 3d keypoints of the box.

    TODO(pablovela5620): Once #1581 is resolved this can be removed
    """
    length, height, width = half_size.tolist()
    center = np.reshape(transform, (-1, 3))
    center = center.reshape(3)
    x_corners = [length, length, -length, -length, length, length, -length, -length]
    y_corners = [height, -height, -height, height, height, -height, -height, height]
    z_corners = [width, width, width, width, -width, -width, -width, -width]
    corners_3d = np.dot(np.transpose(rotation), np.vstack([x_corners, y_corners, z_corners]))

    corners_3d[0, :] += center[0]
    corners_3d[1, :] += center[1]
    corners_3d[2, :] += center[2]
    bbox3d_raw = np.transpose(corners_3d)
    return bbox3d_raw


def log_line_segments(entity_path: str, bboxes_2d_filtered: npt.NDArray[np.float64], color: Color, label: str) -> None:
    """
    Generates line segments for each object's bounding box in 2d.

    Box corner order that we return is of the format below:
      6 -------- 7
     /|         /|
    5 -------- 4 .
    | |        | |
    . 2 -------- 3
    |/         |/
    1 -------- 0

    TODO(pablovela5620): Once #1581 is resolved this can be removed

    :param bboxes_2d_filtered:
        A numpy array of shape (8, 2), representing the filtered 2D keypoints of the 3D bounding boxes.
    :return: A numpy array of shape (24, 2), representing the line segments for each object's bounding boxes.
             Even and odd indices represent the start and end points of each line segment respectively.
    """

    # Calculate the centroid of the 2D keypoints
    valid_points = bboxes_2d_filtered[~np.isnan(bboxes_2d_filtered).any(axis=1)]

    # log centroid and add label so that object label is visible in the 2d view
    if valid_points.size > 0:
        centroid = valid_points.mean(axis=0)
        viewer.log_point(f"{entity_path}/centroid", centroid, color=color, label=label)
    else:
        pass

    # fmt: off
    segments = np.array([
        # bottom of bbox
        bboxes_2d_filtered[0], bboxes_2d_filtered[1],
        bboxes_2d_filtered[1], bboxes_2d_filtered[2],
        bboxes_2d_filtered[2], bboxes_2d_filtered[3],
        bboxes_2d_filtered[3], bboxes_2d_filtered[0],

        # top of bbox
        bboxes_2d_filtered[4], bboxes_2d_filtered[5],
        bboxes_2d_filtered[5], bboxes_2d_filtered[6],
        bboxes_2d_filtered[6], bboxes_2d_filtered[7],
        bboxes_2d_filtered[7], bboxes_2d_filtered[4],

        # sides of bbox
        bboxes_2d_filtered[0], bboxes_2d_filtered[4],
        bboxes_2d_filtered[1], bboxes_2d_filtered[5],
        bboxes_2d_filtered[2], bboxes_2d_filtered[6],
        bboxes_2d_filtered[3], bboxes_2d_filtered[7]
                         ], dtype=np.float32)

    viewer.log_line_segments(entity_path, segments, color=color)


def project_3d_bboxes_to_2d_keypoints(
    bboxes_3d: npt.NDArray[np.float64],
    camera_from_world: Tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]],
    intrinsic: npt.NDArray[np.float64],
    img_width: int,
    img_height: int,
) -> npt.NDArray[np.float64]:
    """
    Returns 2D keypoints of the 3D bounding box in the camera view.

    TODO(pablovela5620): Once #1581 is resolved this can be removed
    Args:
        bboxes_3d: (nObjects, 8, 3) containing the 3D bounding box keypoints in world frame.
        camera_from_world: Tuple containing the camera translation and rotation_quaternion in world frame.
        intrinsic: (3,3) containing the camera intrinsic matrix.
        img_width: Width of the image.
        img_height: Height of the image.

    Returns
    -------
    bboxes_2d_filtered:
        A numpy array of shape (nObjects, 8, 2), representing the 2D keypoints of the 3D bounding boxes. That
        are within the image frame.
    """

    translation, rotation_q = camera_from_world
    rotation = R.from_quat(rotation_q)

    # Transform 3D keypoints from world to camera frame
    world_to_camera_rotation = rotation.as_matrix()
    world_to_camera_translation = translation.reshape(3, 1)
    # Tile translation to match bounding box shape, (nObjects, 1, 3)
    world_to_camera_translation_tiled = np.tile(world_to_camera_translation.T, (bboxes_3d.shape[0], 1, 1))
    # Transform 3D bounding box keypoints from world to camera frame to filter out points behind the camera
    camera_points = (
        np.einsum("ij,afj->afi", world_to_camera_rotation, bboxes_3d[..., :3]) + world_to_camera_translation_tiled
    )
    # Check if the points are in front of the camera
    depth_mask = camera_points[..., 2] > 0
    # convert to transformation matrix shape of (3, 4)
    world_to_camera = np.hstack([world_to_camera_rotation, world_to_camera_translation])
    transformation_matrix = intrinsic @ world_to_camera
    # add batch dimension to match bounding box shape, (nObjects, 3, 4)
    transformation_matrix = np.tile(transformation_matrix, (bboxes_3d.shape[0], 1, 1))
    # bboxes_3d: [nObjects, 8, 3] -> [nObjects, 8, 4] to allow for batch projection
    bboxes_3d = np.concatenate([bboxes_3d, np.ones((bboxes_3d.shape[0], bboxes_3d.shape[1], 1))], axis=-1)
    # Apply depth mask to filter out points behind the camera
    bboxes_3d[~depth_mask] = np.nan
    # batch projection of points using einsum
    bboxes_2d = np.einsum("vab,fnb->vfna", transformation_matrix, bboxes_3d)
    bboxes_2d = bboxes_2d[..., :2] / bboxes_2d[..., 2:]
    # nViews irrelevant, squeeze out
    bboxes_2d = bboxes_2d[0]

    # Filter out keypoints that are not within the frame
    mask_x = (bboxes_2d[:, :, 0] >= 0) & (bboxes_2d[:, :, 0] < img_width)
    mask_y = (bboxes_2d[:, :, 1] >= 0) & (bboxes_2d[:, :, 1] < img_height)
    mask = mask_x & mask_y
    bboxes_2d_filtered = np.where(mask[..., np.newaxis], bboxes_2d, np.nan)

    return bboxes_2d_filtered


def log_camera(
    intri_path: Path,
    frame_id: str,
    poses_from_traj: Dict[str, Tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]]],
    entity_id: str,
    bboxes: npt.NDArray[np.float64],
    bbox_labels: List[str],
    colors: List[Color],
) -> None:
    """Logs camera transform and 3D bounding boxes in the image frame."""
    w, h, fx, fy, cx, cy = np.loadtxt(intri_path)
    intrinsic = np.array([[fx, 0, cx], [0, fy, cy], [0, 0, 1]])
    camera_from_world = poses_from_traj[frame_id]

    # TODO(pablovela5620): Once #1581 is resolved this can be removed
    # Project 3D bounding boxes into 2D image
    bboxes_2d = project_3d_bboxes_to_2d_keypoints(bboxes, camera_from_world, intrinsic, img_width=w, img_height=h)
    # clear previous centroid labels
    viewer.log_cleared(f"{entity_id}/bbox-2d-segments", recursive=True)
    # Log line segments for each bounding box in the image
    for i, (label, bbox_2d) in enumerate(zip(bbox_labels, bboxes_2d)):
        log_line_segments(f"{entity_id}/bbox-2d-segments/{label}", bbox_2d.reshape(-1, 2), colors[i], label)

    viewer.log_rigid3(
        # pathlib makes it easy to get the parent, but log_rigid requires a string
        str(PosixPath(entity_id).parent),
        child_from_parent=camera_from_world,
        xyz="RDF",  # X=Right, Y=Down, Z=Forward
    )
    viewer.log_pinhole(f"{entity_id}", child_from_parent=intrinsic, width=w, height=h)


def read_camera_from_world(traj_string: str) -> Tuple[str, Tuple[npt.NDArray[np.float64], npt.NDArray[np.float64]]]:
    """
    Reads out camera_from_world transform from trajectory string.

    Args:
    ----
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
    """Finds the closest frame id to the target id."""
    target_value = float(target_id)
    closest_id = min(frame_ids.keys(), key=lambda x: abs(float(x) - target_value))
    return closest_id


def log_arkit(recording_path: Path, include_highres: bool) -> None:
    """
    Logs ARKit recording data using Rerun.

    Args:
    ----
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
    camera_from_world_dict = {}
    with open(traj_path, "r", encoding="utf-8") as f:
        trajectory = f.readlines()

    for line in trajectory:
        timestamp, camera_from_world = read_camera_from_world(line)
        # round timestamp to 3 decimal places as seen in the original repo here
        # https://github.com/apple/ARKitScenes/blob/e2e975128a0a9695ea56fa215fe76b4295241538/threedod/benchmark_scripts/utils/tenFpsDataLoader.py#L247
        timestamp = f"{round(float(timestamp), 3):.3f}"
        camera_from_world_dict[timestamp] = camera_from_world

    viewer.log_view_coordinates("world", up="+Z", right_handed=True, timeless=True)
    ply_path = recording_path / f"{recording_path.stem}_3dod_mesh.ply"
    print(f"Loading {ply_path}…")
    assert os.path.isfile(ply_path), f"Failed to find {ply_path}"

    mesh_ply = trimesh.load(str(ply_path))
    viewer.log_mesh(
        "world/mesh",
        positions=mesh_ply.vertices,
        indices=mesh_ply.faces,
        vertex_colors=mesh_ply.visual.vertex_colors,
        timeless=True,
    )

    # load the obb annotations and log them in the world frame
    bbox_annotations_path = recording_path / f"{recording_path.stem}_3dod_annotation.json"
    annotation = load_json(bbox_annotations_path)
    bboxes_3d, bbox_labels, colors_list = log_annotated_bboxes(annotation)

    lowres_posed_entity_id = "world/camera_posed_lowres/image_posed_lowres"
    highres_entity_id = "world/camera_highres/image_highres"

    print("Processing frames…")
    for frame_timestamp in tqdm(lowres_frame_ids):
        # frame_id is equivalent to timestamp
        viewer.set_time_seconds("time", float(frame_timestamp))
        # load the lowres image and depth
        bgr = cv2.imread(f"{lowres_image_dir}/{video_id}_{frame_timestamp}.png")
        rgb = cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB)
        depth = cv2.imread(f"{lowres_depth_dir}/{video_id}_{frame_timestamp}.png", cv2.IMREAD_ANYDEPTH)

        high_res_exists: bool = (image_dir / f"{video_id}_{frame_timestamp}.png").exists() and include_highres

        # Log the camera transforms:
        if frame_timestamp in camera_from_world_dict:
            lowres_intri_path = lowres_intrinsics_dir / f"{video_id}_{frame_timestamp}.pincam"
            log_camera(
                lowres_intri_path,
                frame_timestamp,
                camera_from_world_dict,
                lowres_posed_entity_id,
                bboxes_3d,
                bbox_labels,
                colors_list,
            )

            viewer.log_image(f"{lowres_posed_entity_id}/rgb", rgb)
            viewer.log_depth_image(f"{lowres_posed_entity_id}/depth", depth, meter=1000)

        # log the high res camera
        if high_res_exists:
            viewer.set_time_seconds("time high resolution", float(frame_timestamp))
            # only low res camera has a trajectory, high res does not so need to find the closest low res frame id
            closest_lowres_frame_id = find_closest_frame_id(frame_timestamp, camera_from_world_dict)
            highres_intri_path = intrinsics_dir / f"{video_id}_{frame_timestamp}.pincam"
            log_camera(
                highres_intri_path,
                closest_lowres_frame_id,
                camera_from_world_dict,
                highres_entity_id,
                bboxes_3d,
                bbox_labels,
                colors_list,
            )

            # load the highres image and depth if they exist
            highres_bgr = cv2.imread(f"{image_dir}/{video_id}_{frame_timestamp}.png")
            highres_depth = cv2.imread(f"{depth_dir}/{video_id}_{frame_timestamp}.png", cv2.IMREAD_ANYDEPTH)

            highres_rgb = cv2.cvtColor(highres_bgr, cv2.COLOR_BGR2RGB)
            viewer.log_image(f"{highres_entity_id}/rgb", highres_rgb)
            viewer.log_depth_image(f"{highres_entity_id}/depth", highres_depth, meter=1000)


def main() -> None:
    parser = argparse.ArgumentParser(description="Visualizes the ARKitScenes dataset using the Rerun SDK.")
    parser.add_argument(
        "--video-id",
        type=str,
        choices=AVAILABLE_RECORDINGS,
        default=AVAILABLE_RECORDINGS[0],
        help="Video ID of the ARKitScenes Dataset",
    )
    parser.add_argument(
        "--include-highres",
        action="store_true",
        help="Include the high resolution camera and depth images",
    )
    viewer.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    viewer.script_setup(args, "arkitscenes")
    recording_path = ensure_recording_available(args.video_id, args.include_highres)
    log_arkit(recording_path, args.include_highres)

    viewer.script_teardown(args)


if __name__ == "__main__":
    main()
