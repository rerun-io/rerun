#!/usr/bin/env python3

"""
Generate SDFs for arbitrary meshes and visualize the results using the Rerun SDK.

Using both traditional methods as well as the one described in the DeepSDF paper ([1]).

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

Run:
```sh
# assuming your virtual env is up
examples/python/deep_sdf/main.py
```
"""


import argparse
import os
from pathlib import Path
from timeit import default_timer as timer
from typing import Tuple, cast

import mesh_to_sdf
import numpy as np
import numpy.typing as npt
import depthai_viewer as viewer
import trimesh
from download_dataset import AVAILABLE_MESHES, ensure_mesh_downloaded
from trimesh import Trimesh

CACHE_DIR = Path(os.path.dirname(__file__)) / "cache"


def log_timing_decorator(objpath: str, level: str):  # type: ignore[no-untyped-def]
    """Times the inner method using `timeit`, and logs the result using Rerun."""

    def inner(func):  # type: ignore[no-untyped-def]
        def wrapper(*args, **kwargs):  # type: ignore[no-untyped-def]
            now = timer()
            result = func(*args, **kwargs)
            elapsed_ms = (timer() - now) * 1_000.0
            viewer.log_text_entry(objpath, f"execution took {elapsed_ms:.1f}ms", level=level)
            return result

        return wrapper

    return inner


# TODO(cmc): This really should be the job of the SDK.
def get_mesh_format(mesh: Trimesh) -> viewer.MeshFormat:
    ext = Path(mesh.metadata["file_name"]).suffix.lower()
    try:
        return {
            ".glb": viewer.MeshFormat.GLB,
            # ".gltf": MeshFormat.GLTF,
            ".obj": viewer.MeshFormat.OBJ,
        }[ext]
    except Exception:
        raise ValueError(f"unknown file extension: {ext}")


def read_mesh(path: Path) -> Trimesh:
    print(f"loading mesh {path}…")
    mesh = trimesh.load(path)
    return cast(Trimesh, mesh)


@log_timing_decorator("global/voxel_sdf", viewer.LogLevel.DEBUG)  # type: ignore[misc]
def compute_voxel_sdf(mesh: Trimesh, resolution: int) -> npt.NDArray[np.float32]:
    print("computing voxel-based SDF")
    voxvol = np.array(mesh_to_sdf.mesh_to_voxels(mesh, voxel_resolution=resolution), dtype=np.float32)
    return voxvol


@log_timing_decorator("global/sample_sdf", viewer.LogLevel.DEBUG)  # type: ignore[misc]
def compute_sample_sdf(mesh: Trimesh, num_points: int) -> Tuple[npt.NDArray[np.float32], npt.NDArray[np.float32]]:
    print("computing sample-based SDF")
    points, sdf, _ = mesh_to_sdf.sample_sdf_near_surface(mesh, number_of_points=num_points, return_gradients=True)
    return (points, sdf)


@log_timing_decorator("global/log_mesh", viewer.LogLevel.DEBUG)  # type: ignore[misc]
def log_mesh(path: Path, mesh: Trimesh) -> None:
    # Internally, `mesh_to_sdf` will normalize everything to a unit sphere centered around the
    # center of mass.
    # We need to compute a proper transform to map the mesh we're logging with the point clouds
    # that `mesh_to_sdf` returns.
    bs1 = mesh.bounding_sphere
    bs2 = mesh_to_sdf.scale_to_unit_sphere(mesh).bounding_sphere
    mesh_format = get_mesh_format(mesh)

    with open(path, mode="rb") as file:
        scale = bs2.scale / bs1.scale
        center = bs2.center - bs1.center * scale
        viewer.log_mesh_file(
            "world/mesh",
            mesh_format,
            file.read(),
            transform=np.array([[scale, 0, 0, center[0]], [0, scale, 0, center[1]], [0, 0, scale, center[2]]]),
        )


def log_sampled_sdf(points: npt.NDArray[np.float32], sdf: npt.NDArray[np.float32]) -> None:
    # viewer.log_view_coordinates("world", up="+Y", timeless=True # TODO(cmc): depends on the mesh really
    viewer.log_annotation_context("world/sdf", [(0, "inside", (255, 0, 0)), (1, "outside", (0, 255, 0))], timeless=False)
    viewer.log_points("world/sdf/points", points, class_ids=np.array(sdf > 0, dtype=np.uint8))

    outside = points[sdf > 0]
    viewer.log_text_entry(
        "world/sdf/inside/logs",
        f"{len(points) - len(outside)} points inside ({len(points)} total)",
        level=viewer.LogLevel.TRACE,
    )
    viewer.log_text_entry(
        "world/sdf/outside/logs", f"{len(outside)} points outside ({len(points)} total)", level=viewer.LogLevel.TRACE
    )


def log_volumetric_sdf(voxvol: npt.NDArray[np.float32]) -> None:
    names = ["width", "height", "depth"]
    viewer.log_tensor("tensor", voxvol, names=names)


@log_timing_decorator("global/log_mesh", viewer.LogLevel.DEBUG)  # type: ignore[misc]
def compute_and_log_volumetric_sdf(mesh_path: Path, mesh: Trimesh, resolution: int) -> None:
    os.makedirs(CACHE_DIR, exist_ok=True)
    basename = os.path.basename(mesh_path)
    voxvol_path = f"{CACHE_DIR}/{basename}.voxvol.{resolution}.npy"
    try:
        with open(voxvol_path, "rb") as f:
            voxvol = np.load(voxvol_path)
            viewer.log_text_entry("global", "loading volumetric SDF from cache")
    except Exception:
        voxvol = compute_voxel_sdf(mesh, resolution)

    log_volumetric_sdf(voxvol)

    with open(voxvol_path, "wb+") as f:
        np.save(f, voxvol)
        viewer.log_text_entry("global", "writing volumetric SDF to cache", level=viewer.LogLevel.DEBUG)


@log_timing_decorator("global/log_mesh", viewer.LogLevel.DEBUG)  # type: ignore[misc]
def compute_and_log_sample_sdf(mesh_path: Path, mesh: Trimesh, num_points: int) -> None:
    basename = os.path.basename(mesh_path)
    points_path = f"{CACHE_DIR}/{basename}.points.{num_points}.npy"
    sdf_path = f"{CACHE_DIR}/{basename}.sdf.npy"

    os.makedirs(CACHE_DIR, exist_ok=True)
    try:
        with open(sdf_path, "rb") as f:
            sdf = np.load(sdf_path)
            viewer.log_text_entry("global", "loading sampled SDF from cache")
        with open(points_path, "rb") as f:
            points = np.load(points_path)
            viewer.log_text_entry("global", "loading point cloud from cache")
    except Exception:
        (points, sdf) = compute_sample_sdf(mesh, num_points)

    log_mesh(mesh_path, mesh)
    log_sampled_sdf(points, sdf)

    with open(points_path, "wb+") as f:
        np.save(f, points)
        viewer.log_text_entry("global", "writing sampled SDF to cache", level=viewer.LogLevel.DEBUG)
    with open(sdf_path, "wb+") as f:
        np.save(f, sdf)
        viewer.log_text_entry("global", "writing point cloud to cache", level=viewer.LogLevel.DEBUG)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Generates SDFs for arbitrary meshes and logs the results using the Rerun SDK."
    )
    parser.add_argument("--resolution", type=int, default=128, help="Specifies the resolution of the voxel volume")
    parser.add_argument(
        "--points", type=int, default=250_000, help="Specifies the number of points for the point cloud"
    )
    parser.add_argument(
        "--mesh",
        type=str,
        choices=AVAILABLE_MESHES,
        default=AVAILABLE_MESHES[0],
        help="The name of the mesh to analyze",
    )
    parser.add_argument(
        "--mesh_path",
        type=Path,
        help="Path to a mesh to analyze. If set, overrides the `--mesh` argument.",
    )
    viewer.script_add_args(parser)
    args, unknown = parser.parse_known_args()
    [__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

    viewer.script_setup(args, "deep_sdf")

    mesh_path = args.mesh_path
    if mesh_path is None:
        mesh_path = ensure_mesh_downloaded(args.mesh)
    mesh = read_mesh(mesh_path)

    compute_and_log_sample_sdf(mesh_path, mesh, args.points)

    compute_and_log_volumetric_sdf(mesh_path, mesh, args.resolution)

    viewer.script_teardown(args)


if __name__ == "__main__":
    main()
