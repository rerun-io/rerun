#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import Any, Tuple

import cv2
import numpy as np
import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rbl
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

lowres_posed_entity_id = "world/camera_lowres"
highres_entity_id = "world/camera_highres"


def load_json(js_path: Path) -> dict[str, Any]:
    with open(js_path) as f:
        json_data: dict[str, Any] = json.load(f)
    return json_data


def log_annotated_bboxes(annotation: dict[str, Any]) -> tuple[npt.NDArray[np.float64], list[str], list[Color]]:
    """
    Logs annotated oriented bounding boxes to Rerun.

    annotation json file
    |  |-- label: object name of bounding box
    |  |-- axesLengths[x, y, z]: size of the origin bounding-box before transforming
    |  |-- centroid[]: the translation matrix (1,3) of bounding-box
    |  |-- normalizedAxes[]: the rotation matrix (3,3) of bounding-box
    """

    for label_info in annotation["data"]:
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
            ),
            timeless=True,
        )


def log_camera(
    intri_path: Path,
    frame_id: str,
    poses_from_traj: dict[str, rr.TranslationRotationScale3D],
    entity_id: str,
) -> None:
    """Logs camera transform and 3D bounding boxes in the image frame."""
    w, h, fx, fy, cx, cy = np.loadtxt(intri_path)
    intrinsic = np.array([[fx, 0, cx], [0, fy, cy], [0, 0, 1]])
    camera_from_world = poses_from_traj[frame_id]

    # clear previous centroid labels
    rr.log(f"{entity_id}/bbox-2D-segments", rr.Clear(recursive=True))

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
    log_annotated_bboxes(annotation)

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

    primary_camera_entity = highres_entity_id if args.include_highres else lowres_posed_entity_id

    rr.script_setup(
        args,
        "rerun_example_arkit_scenes",
        blueprint=rbl.Horizontal(
            rbl.Spatial3DView(name="3D"),
            rbl.Vertical(
                rbl.Tabs(
                    # Note that we re-project the annotations into the 2D views:
                    # For this to work, the origin of the 2D views has to be a pinhole camera,
                    # this way the viewer knows how to project the 3D annotations into the 2D views.
                    rbl.Spatial2DView(
                        name="RGB",
                        origin=primary_camera_entity,
                        contents=[f"{primary_camera_entity}/rgb", "/world/annotations/**"],
                    ),
                    rbl.Spatial2DView(
                        name="Depth",
                        origin=primary_camera_entity,
                        contents=[f"{primary_camera_entity}/depth", "/world/annotations/**"],
                    ),
                ),
                rbl.TextDocumentView(),
            ),
        ),
    )
    recording_path = ensure_recording_available(args.video_id, args.include_highres)
    log_arkit(recording_path, args.include_highres)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
