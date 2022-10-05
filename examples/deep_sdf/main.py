#!/usr/bin/env python3

"""
Generate SDFs for arbitrary meshes using both traditional methods as well as the one described in
the DeepSDF paper ([1]), and visualize the results using the Rerun SDK.

[1] @InProceedings{Park_2019_CVPR,
    author = {
        Park,
        Jeong Joon and Florence,
        Peter and Straub,
        Julian and Newcombe,
        Richard and Lovegrove,
        Steven,
    },
    title = {DeepSDF: Learning Continuous Signed Distance Functions for Shape Representation},
    booktitle = {The IEEE Conference on Computer Vision and Pattern Recognition (CVPR)},
    month = {June},
    year = {2019}
}

Setup:
```sh
./examples/deep_sdf/download_dataset.py
```

Run:
```sh
# assuming your virtual env is up
python3 examples/deep_sdf/main.py examples/deep_sdf/dataset/avocado.glb
```
"""


import argparse
import os

import mesh_to_sdf
import numpy as np
import rerun_sdk as rerun
import trimesh

from pathlib import Path
from timeit import default_timer as timer
from typing import Tuple, cast

from scipy.spatial.transform import Rotation as R
from trimesh import Trimesh
from rerun_sdk import MeshFormat


def announcement(body: str):
    announcement.counter += 1
    rerun.log_text_entry(f"ann/#{announcement.counter}",
                         body,
                         color=[255, 215, 0],
                         timeless=True)
announcement.counter = 0


def log_timing_decorator(objpath: str, level: str):
    """
    Times the inner method using `timeit`, and logs the result using Rerun.
    """
    def inner(func):
        def wrapper(*args, **kwargs):
            now = timer()
            result = func(*args, **kwargs)
            elapsed_ms = (timer() - now) * 1_000.0
            rerun.log_text_entry(objpath,
                                 f"execution took {elapsed_ms}ms",
                                 level=level,
                                 space="autologs")
            return result
        return wrapper
    return inner


# TODO(cmc): This really should be the job of the SDK.
def get_mesh_format(mesh: Trimesh) -> MeshFormat:
    ext = Path(mesh.metadata['file_name']).suffix.lower()
    try:
        return {
            ".glb": MeshFormat.GLB,
            # ".gltf": MeshFormat.GLTF,
            ".obj": MeshFormat.OBJ,
        }[ext]
    except:
        raise ValueError(f"unknown file extension: {ext}")


def read_mesh(path: Path) -> Trimesh:
    print(f"loading mesh {path}…")
    mesh = trimesh.load(path)
    return cast(Trimesh, mesh)


@log_timing_decorator("global/voxel_sdf", "debug")
def compute_voxel_sdf(mesh: Trimesh, resolution: int) -> np.ndarray:
    print("computing voxel-based SDF")
    voxvol = mesh_to_sdf.mesh_to_voxels(mesh, voxel_resolution=resolution)
    return voxvol


@log_timing_decorator("global/sample_sdf", "debug")
def compute_sample_sdf(mesh: Trimesh, num_points: int) -> Tuple[np.ndarray, np.ndarray]:
    print("computing sample-based SDF")
    points, sdf, _ = mesh_to_sdf.sample_sdf_near_surface(mesh,
                                                         number_of_points=num_points,
                                                         return_gradients=True)
    return (points, sdf)


@log_timing_decorator("global/log_mesh", "debug")
def log_mesh(path: Path, mesh: Trimesh):
    # Internally, `mesh_to_sdf` will normalize everything to a unit sphere centered around the
    # center of mass.
    # We need to compute a proper transform to map the mesh we're logging with the point clouds
    # that `mesh_to_sdf` returns.
    bs1 = mesh.bounding_sphere
    bs2 = mesh_to_sdf.scale_to_unit_sphere(mesh).bounding_sphere

    with open(path, mode='rb') as file:
        scale = bs2.scale / bs1.scale
        center = bs2.center - bs1.center * scale
        rerun.log_mesh_file("mesh", mesh_format, file.read(), space="world",
                            transform=np.array([[scale, 0, 0, center[0]],
                                                [0, scale, 0, center[1]],
                                                [0, 0, scale, center[2]]]))


def log_sampled_sdf(points: np.ndarray, sdf: np.ndarray):
    rerun.set_space_up("world", [0, 1, 0]) # TODO(cmc): depends on the mesh really

    inside = points[sdf <= 0]
    rerun.log_text_entry("sdf/inside/logs",
                         f"{len(inside)} points inside ({len(points)} total)",
                         level="trace")
    rerun.log_points("sdf/inside",
                     points[sdf <= 0],
                     colors=np.array([255, 0, 0, 255]),
                     space="world")

    outside = points[sdf > 0]
    rerun.log_text_entry("sdf/outside/logs",
                  f"{len(outside)} points outside ({len(points)} total)",
                  level="trace")
    rerun.log_points("sdf/outside",
                     points[sdf > 0],
                     colors=np.array([0, 255, 0, 255]),
                     space="world")


def log_volumetric_sdf(voxvol: np.ndarray):
    names = ["width", "height", "depth"]
    rerun.log_tensor("sdf/tensor", voxvol, names=names, space="tensor")


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Generates SDFs for arbitrary meshes and logs the results using the Rerun SDK.')
    parser.add_argument('--headless', action='store_true',
                        help="Don't show GUI")
    parser.add_argument('--connect', dest='connect', action='store_true',
                        help='Connect to an external viewer')
    parser.add_argument('--addr', type=str, default=None,
                        help='Connect to this ip:port')
    parser.add_argument('--save', type=str, default=None,
                        help='Save data to a .rrd file at this path')
    parser.add_argument('--resolution', type=int, default=128,
                        help='Specifies the resolution of the voxel volume')
    parser.add_argument('--points', type=int, default=250_000,
                        help='Specifies the number of points for the point cloud')
    parser.add_argument('path', type=Path,
                        help='Mesh to log (e.g. `dataset/avocado.glb`)')
    parser.add_argument(
        "--serve",
        dest="serve",
        action="store_true",
        help="Serve a web viewer (WARNING: experimental feature)",
    )
    args = parser.parse_args()

    if args.serve:
        rerun.serve()
    elif args.connect:
        # Send logging data to separate `rerun` process.
        # You can omit the argument to connect to the default address,
        # which is `127.0.0.1:9876`.
        rerun.connect(args.addr)

    cachedir = Path(os.path.dirname(__file__)).joinpath("cache")
    os.makedirs(cachedir, exist_ok=True)

    now = timer()

    path = args.path
    mesh = read_mesh(path)
    mesh_format = get_mesh_format(mesh)

    basename = os.path.basename(path)
    points_path = f"{cachedir}/{basename}.points.{args.points}.npy"
    sdf_path = f"{cachedir}/{basename}.sdf.npy"
    voxvol_path = f"{cachedir}/{basename}.voxvol.{args.resolution}.npy"

    try:
        with open(sdf_path, 'rb') as f:
            sdf = np.load(sdf_path)
            rerun.log_text_entry("global", "loading sampled SDF from cache")
        with open(points_path, 'rb') as f:
            points = np.load(points_path)
            rerun.log_text_entry("global", "loading point cloud from cache")
    except:
        (points, sdf) = compute_sample_sdf(mesh, args.points)

    try:
        with open(voxvol_path, 'rb') as f:
            voxvol = np.load(voxvol_path)
            rerun.log_text_entry("global", "loading volumetric SDF from cache")
    except:
        voxvol = compute_voxel_sdf(mesh, args.resolution)

    # TODO(cmc): really could use some structured logging here!
    announcement(f"starting DeepSDF logger")
    announcement(f"point cloud size: {args.points}")
    announcement(f"voxel resolution: {args.resolution}")

    log_mesh(path, mesh)
    log_sampled_sdf(points, sdf)
    log_volumetric_sdf(voxvol)

    elapsed_ms = (timer() - now) * 1_000.0
    announcement(f"SDFs computed and logged in {elapsed_ms}ms")

    with open(points_path, 'wb+') as f:
        np.save(f, points)
        rerun.log_text_entry("global", "writing sampled SDF to cache", level="debug")
    with open(sdf_path, 'wb+') as f:
        np.save(f, sdf)
        rerun.log_text_entry("global", "writing point cloud to cache", level="debug")
    with open(voxvol_path, 'wb+') as f:
        np.save(f, voxvol)
        rerun.log_text_entry("global", "writing volumetric SDF to cache", level="debug")

    if args.serve:
        print("Sleeping while serving the web viewer. Abort with Ctrl-C")
        try:
            from time import sleep
            sleep(100_000)
        except:
            pass
    elif args.save is not None:
        rerun.save(args.save)
    elif args.headless:
        pass
    elif not args.connect:
        rerun.show()
