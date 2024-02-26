#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import Any, Tuple

import cv2
import matplotlib.pyplot as plt
import numpy as np
import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk
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


DESCRIPTION = """
# ARKit Scenes
This example visualizes the [ARKitScenes dataset](https://github.com/apple/ARKitScenes/) using Rerun. The dataset
contains color images, depth images, the reconstructed mesh, and labeled bounding boxes around furniture.

## How it was made
The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/arkit_scenes/main.py).

### Moving RGB-D camera
To log a moving RGB-D camera we need to log four objects: the pinhole camera (intrinsics), the camera pose
(extrinsics), the color image and the depth image.

The [rr.Pinhole archetype](https://www.rerun.io/docs/reference/types/archetypes/pinhole) is logged to
[world/camera_lowres](recording://world/camera_lowres) to define the intrinsics of the camera. This
determines how to go from the 3D camera frame to the 2D image plane. The extrinsics are logged as an
[rr.Transform3D archetype](https://www.rerun.io/docs/reference/types/archetypes/transform3d) to the
[same entity world/camera_lowres](recording://world/camera_lowres). Note that we could also log the extrinsics to
`world/camera` and the intrinsics to `world/camera/image` instead. Here, we log both on the same entity path to keep
the paths shorter.

The RGB image is logged as an
[rr.Image archetype](https://www.rerun.io/docs/reference/types/archetypes/image) to the
[world/camera_lowres/rgb entity](recording://world/camera_lowres/rgb) as a child of the intrinsics + extrinsics
entity described in the previous paragraph. Similarly the depth image is logged as an
[rr.DepthImage archetype](https://www.rerun.io/docs/reference/types/archetypes/depth_image) to
[world/camera_lowres/depth](recording://world/camera_lowres/depth).

### Ground-truth mesh
The mesh is logged as an [rr.Mesh3D archetype](https://www.rerun.io/docs/reference/types/archetypes/mesh3d).
In this case the mesh is composed of mesh vertices, indices (i.e., which vertices belong to the same face), and vertex
colors. Given a `trimesh.Trimesh` the following call is used to log it to Rerun
```python
rr.log(
    "world/mesh",
    rr.Mesh3D(
        vertex_positions=mesh.vertices,
        vertex_colors=mesh.visual.vertex_colors,
        indices=mesh.faces,
    ),
    timeless=True,
)
```
Here, the mesh is logged to the [world/mesh entity](recording://world/mesh) and is marked as timeless, since it does not
change in the context of this visualization.

### 3D bounding boxes
The bounding boxes around the furniture is visualized by logging the
[rr.Boxes3D archetype](https://www.rerun.io/docs/reference/types/archetypes/boxes3d). In this example, each
bounding box is logged as a separate entity to the common [world/annotations](recording://world/annotations) parent.
""".strip()


def load_json(js_path: Path) -> dict[str, Any]:
    with open(js_path) as f:
        json_data: dict[str, Any] = json.load(f)
    return json_data


def log_annotated_bboxes(annotation: dict[str, Any]) -> tuple[npt.NDArray[np.float64], list[str], list[Color]]:
    """
    Logs annotated oriented bounding boxes to Rerun.

    We currently calculate and return the 3D bounding boxes keypoints, labels, and colors for each object to log them in
    each camera frame TODO(#3412): once resolved this can be removed.

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
    # TODO(#3412, #1728): once resolved this can be removed
    color_positions = np.linspace(0, 1, num_objects)
    colormap = plt.colormaps["viridis"]
    colors = [colormap(pos) for pos in color_positions]

    for i, label_info in enumerate(annotation["data"]):
        uid = label_info["uid"]
        label = label_info["label"]

        half_size = 0.5 * np.array(label_info["segments"]["obbAligned"]["axesLengths"]).reshape(-1, 3)[0]
        centroid = np.array(label_info["segments"]["obbAligned"]["centroid"]).reshape(-1, 3)[0]
        rotation = np.array(label_info["segments"]["obbAligned"]["normalizedAxes"]).reshape(3, 3)

        rot = R.from_matrix(rotation).inv()

        rr.log(
            f"world/annotations/box-{uid}-{label}",
            rr.Boxes3D(
                half_sizes=half_size,
                centers=centroid,
                rotations=rr.Quaternion(xyzw=rot.as_quat()),
                labels=label,
                colors=colors[i],
            ),
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

    TODO(#3412): once resolved this can be removed
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

    TODO(#3412): once resolved this can be removed

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
        rr.log(f"{entity_path}/centroid", rr.Points2D(centroid, colors=color, labels=label))
    else:
        pass

    segments = [
        # bottom of bbox
        [bboxes_2d_filtered[0], bboxes_2d_filtered[1]],
        [bboxes_2d_filtered[1], bboxes_2d_filtered[2]],
        [bboxes_2d_filtered[2], bboxes_2d_filtered[3]],
        [bboxes_2d_filtered[3], bboxes_2d_filtered[0]],
        # top of bbox
        [bboxes_2d_filtered[4], bboxes_2d_filtered[5]],
        [bboxes_2d_filtered[5], bboxes_2d_filtered[6]],
        [bboxes_2d_filtered[6], bboxes_2d_filtered[7]],
        [bboxes_2d_filtered[7], bboxes_2d_filtered[4]],
        # sides of bbox
        [bboxes_2d_filtered[0], bboxes_2d_filtered[4]],
        [bboxes_2d_filtered[1], bboxes_2d_filtered[5]],
        [bboxes_2d_filtered[2], bboxes_2d_filtered[6]],
        [bboxes_2d_filtered[3], bboxes_2d_filtered[7]],
    ]

    rr.log(entity_path, rr.LineStrips2D(segments, colors=color))


def project_3d_bboxes_to_2d_keypoints(
    bboxes_3d: npt.NDArray[np.float64],
    camera_from_world: rr.TranslationRotationScale3D,
    intrinsic: npt.NDArray[np.float64],
    img_width: int,
    img_height: int,
) -> npt.NDArray[np.float64]:
    """
    Returns 2D keypoints of the 3D bounding box in the camera view.

    TODO(#3412): once resolved this can be removed
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

    translation, rotation_q = camera_from_world.translation, camera_from_world.rotation
    # We know we stored the rotation as a quaternion, so extract it again.
    # TODO(#3467): This shouldn't directly access rotation.inner
    rotation = R.from_quat(rotation_q.inner)  # type: ignore[union-attr]

    # Transform 3D keypoints from world to camera frame
    world_to_camera_rotation = rotation.as_matrix()
    world_to_camera_translation = np.array(translation).reshape(3, 1)
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
    poses_from_traj: dict[str, rr.TranslationRotationScale3D],
    entity_id: str,
    bboxes: npt.NDArray[np.float64],
    bbox_labels: list[str],
    colors: list[Color],
) -> None:
    """Logs camera transform and 3D bounding boxes in the image frame."""
    w, h, fx, fy, cx, cy = np.loadtxt(intri_path)
    intrinsic = np.array([[fx, 0, cx], [0, fy, cy], [0, 0, 1]])
    camera_from_world = poses_from_traj[frame_id]

    # TODO(#3412): once resolved this can be removed
    # Project 3D bounding boxes into 2D image
    bboxes_2d = project_3d_bboxes_to_2d_keypoints(bboxes, camera_from_world, intrinsic, img_width=w, img_height=h)

    # clear previous centroid labels
    rr.log(f"{entity_id}/bbox-2d-segments", rr.Clear(recursive=True))

    # Log line segments for each bounding box in the image
    for i, (label, bbox_2d) in enumerate(zip(bbox_labels, bboxes_2d)):
        log_line_segments(f"{entity_id}/bbox-2d-segments/{label}", bbox_2d.reshape(-1, 2), colors[i], label)

    # pathlib makes it easy to get the parent, but log methods requires a string
    rr.log(entity_id, rr.Transform3D(transform=camera_from_world))
    rr.log(entity_id, rr.Pinhole(image_from_camera=intrinsic, resolution=[w, h]))


def read_camera_from_world(traj_string: str) -> tuple[str, rr.TranslationRotationScale3D]:
    """
    Reads out camera_from_world transform from trajectory string.

    Args:
    ----
    traj_string:
        A space-delimited file where each line represents a camera position at a particular timestamp.
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
    AssertionError:
        If the input string does not contain 7 tokens.

    """
    tokens = traj_string.split()  # Split the input string into tokens
    assert len(tokens) == 7, f"Input string must have 7 tokens, but found {len(tokens)}."
    ts: str = tokens[0]  # Extract timestamp from the first token

    # Extract rotation from the second to fourth tokens
    angle_axis = [float(tokens[1]), float(tokens[2]), float(tokens[3])]
    rotation = R.from_rotvec(np.asarray(angle_axis))

    # Extract translation from the fifth to seventh tokens
    translation = np.asarray([float(tokens[4]), float(tokens[5]), float(tokens[6])])

    # Create tuple in format log_transform3d expects
    camera_from_world = rr.TranslationRotationScale3D(
        translation, rr.Quaternion(xyzw=rotation.as_quat()), from_parent=True
    )

    return (ts, camera_from_world)


def find_closest_frame_id(target_id: str, frame_ids: dict[str, Any]) -> str:
    """Finds the closest frame id to the target id."""
    target_value = float(target_id)
    closest_id = min(frame_ids.keys(), key=lambda x: abs(float(x) - target_value))
    return closest_id


def log_arkit(recording_path: Path, include_highres: bool) -> None:
    """
    Logs ARKit recording data using Rerun.

    Args:
    ----
    recording_path (Path):
        The path to the ARKit recording.

    include_highres (bool):
        Whether to include high resolution data.

    Returns
    -------
    None

    """
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), timeless=True)

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
    with open(traj_path, encoding="utf-8") as f:
        trajectory = f.readlines()

    for line in trajectory:
        timestamp, camera_from_world = read_camera_from_world(line)
        # round timestamp to 3 decimal places as seen in the original repo here
        # https://github.com/apple/ARKitScenes/blob/e2e975128a0a9695ea56fa215fe76b4295241538/threedod/benchmark_scripts/utils/tenFpsDataLoader.py#L247
        timestamp = f"{round(float(timestamp), 3):.3f}"
        camera_from_world_dict[timestamp] = camera_from_world

    rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)
    ply_path = recording_path / f"{recording_path.stem}_3dod_mesh.ply"
    print(f"Loading {ply_path}…")
    assert os.path.isfile(ply_path), f"Failed to find {ply_path}"

    mesh = trimesh.load(str(ply_path))
    rr.log(
        "world/mesh",
        rr.Mesh3D(
            vertex_positions=mesh.vertices,
            vertex_colors=mesh.visual.vertex_colors,
            indices=mesh.faces,
        ),
        timeless=True,
    )

    # load the obb annotations and log them in the world frame
    bbox_annotations_path = recording_path / f"{recording_path.stem}_3dod_annotation.json"
    annotation = load_json(bbox_annotations_path)
    bboxes_3d, bbox_labels, colors_list = log_annotated_bboxes(annotation)

    lowres_posed_entity_id = "world/camera_lowres"
    highres_entity_id = "world/camera_highres"

    print("Processing frames…")
    for frame_timestamp in tqdm(lowres_frame_ids):
        # frame_id is equivalent to timestamp
        rr.set_time_seconds("time", float(frame_timestamp))
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

            rr.log(f"{lowres_posed_entity_id}/rgb", rr.Image(rgb).compress(jpeg_quality=95))
            rr.log(f"{lowres_posed_entity_id}/depth", rr.DepthImage(depth, meter=1000))

        # log the high res camera
        if high_res_exists:
            rr.set_time_seconds("time high resolution", float(frame_timestamp))
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

            rr.log(f"{highres_entity_id}/rgb", rr.Image(highres_rgb).compress(jpeg_quality=75))
            rr.log(f"{highres_entity_id}/depth", rr.DepthImage(highres_depth, meter=1000))


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
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_arkit_scenes")
    recording_path = ensure_recording_available(args.video_id, args.include_highres)
    log_arkit(recording_path, args.include_highres)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
