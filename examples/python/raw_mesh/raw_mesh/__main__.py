#!/usr/bin/env python3
"""
Shows how to use the Rerun SDK to log raw 3D meshes (so-called "triangle soups") and their transform hierarchy.

This example demonstrates how to construct mesh data programmatically from scratch,
including vertices, normals, colors, texture coordinates, and materials.

If you want to log existing mesh files (like GLTF, OBJ, STL, etc.), use the
[`Asset3D`](https://rerun.io/docs/reference/types/archetypes/asset3d) archetype instead.
"""

from __future__ import annotations

import argparse
from typing import Any

import numpy as np
import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb

DESCRIPTION = """
# Raw meshes
This example shows how to construct and log 3D meshes programmatically from scratch, including their transform hierarchy.

It demonstrates how to create vertices, normals, colors, texture coordinates, and materials for various geometric primitives.

The full source code for this example is available [on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/raw_mesh).
"""


def generate_checkerboard_texture(size: int = 32, checker_size: int = 4) -> npt.NDArray[np.uint8]:
    """
    Generate a simple checkerboard texture.

    Args:
        size: Texture size in pixels (width and height)
        checker_size: Size of each checker square in pixels

    Returns:
        RGB texture array of shape (size, size, 3)
    """
    texture = np.zeros((size, size, 3), dtype=np.uint8)
    for i in range(size):
        for j in range(size):
            if ((i // checker_size) + (j // checker_size)) % 2 == 0:
                texture[i, j] = [200, 200, 200]  # Light gray
            else:
                texture[i, j] = [50, 50, 50]     # Dark gray
    return texture


def generate_sphere(subdivisions: int = 32) -> dict[str, Any]:
    """
    Generate a UV sphere.

    Creates a sphere using latitude/longitude parameterization.
    Returns base geometry with positions, normals, UV coordinates, and per-vertex colors.

    Args:
        subdivisions: Number of latitude subdivisions (longitude will be 2x this)

    Returns a dictionary with vertex_positions, vertex_normals, vertex_texcoords,
            vertex_colors, and triangle_indices.
    """
    if subdivisions < 3:
        msg = "Sphere subdivisions must be at least 3 to form a valid mesh."
        raise ValueError(msg)

    lat_divs = subdivisions
    lon_divs = subdivisions * 2

    vertices = []
    normals = []
    texcoords = []
    colors = []

    # Generate vertices, normals, UVs, and colors
    for lat in range(lat_divs + 1):
        theta = np.pi * lat / lat_divs  # 0 to pi
        sin_theta = np.sin(theta)
        cos_theta = np.cos(theta)
        v = lat / lat_divs  # V coordinate

        for lon in range(lon_divs):
            phi = 2 * np.pi * lon / lon_divs  # 0 to 2pi
            sin_phi = np.sin(phi)
            cos_phi = np.cos(phi)
            u = lon / lon_divs  # U coordinate

            # Position on unit sphere
            x = sin_theta * cos_phi
            y = cos_theta
            z = sin_theta * sin_phi

            vertices.append([x * 0.5, y * 0.5, z * 0.5])  # Scale to radius 0.5
            normals.append([x, y, z])  # Normal is same as position for unit sphere
            texcoords.append([u, v])

            # Generate per-vertex colors based on position (creates a nice gradient)
            r = int(128 + 127 * x)
            g = int(128 + 127 * y)
            b = int(128 + 127 * z)
            colors.append([r, g, b])

    vertices = np.array(vertices, dtype=np.float32)
    normals = np.array(normals, dtype=np.float32)
    texcoords = np.array(texcoords, dtype=np.float32)
    colors = np.array(colors, dtype=np.uint8)

    # Generate triangle indices
    indices = []
    for lat in range(lat_divs):
        next_lat_row_start = (lat + 1) * lon_divs
        curr_lat_row_start = lat * lon_divs

        for lon in range(lon_divs):
            next_lon = (lon + 1) % lon_divs

            first = curr_lat_row_start + lon
            first_next = curr_lat_row_start + next_lon
            second = next_lat_row_start + lon
            second_next = next_lat_row_start + next_lon

            # Two triangles per quad (counter-clockwise winding)
            indices.append([first, second, first_next])
            indices.append([first_next, second, second_next])

    indices = np.array(indices, dtype=np.uint32)

    return {
        "vertex_positions": vertices,
        "vertex_normals": normals,
        "vertex_texcoords": texcoords,
        "vertex_colors": colors,
        "triangle_indices": indices,
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Logs a procedurally generated 3D mesh with various material properties using the Rerun SDK.",
    )
    parser.add_argument(
        "--sphere-subdivisions",
        type=int,
        default=32,
        help="Number of subdivisions for the sphere (default: 32)",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    blueprint = rrb.Horizontal(
        rrb.Spatial3DView(name="Mesh", origin="/world"),
        rrb.TextDocumentView(name="Description", origin="/description"),
        column_shares=[3, 1],
    )

    rr.script_setup(args, "rerun_example_raw_mesh", default_blueprint=blueprint)
    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    # Set coordinate system (right-handed, Y-up)
    rr.log("world", rr.ViewCoordinates.RUB, static=True)

    # Generate base sphere geometry once
    print(f"Generating sphere with {args.sphere_subdivisions} subdivisions...")
    sphere_data = generate_sphere(args.sphere_subdivisions)

    # Instance 1: Vertex colors only (center)
    print("Logging sphere with vertex colors...")
    rr.log("world/sphere/vertex_colors", rr.Transform3D(translation=[0.0, 0.0, 0.0]))
    rr.log(
        "world/sphere/vertex_colors",
        rr.Mesh3D(
            vertex_positions=sphere_data["vertex_positions"],
            vertex_colors=sphere_data["vertex_colors"],
            triangle_indices=sphere_data["triangle_indices"],
        ),
    )

    # Instance 2: Albedo factor (solid color, left)
    print("Logging sphere with albedo factor...")
    rr.log("world/sphere/albedo_factor", rr.Transform3D(translation=[-1.5, 0.0, 0.0]))
    rr.log(
        "world/sphere/albedo_factor",
        rr.Mesh3D(
            vertex_positions=sphere_data["vertex_positions"],
            albedo_factor=np.array([255, 100, 150, 255], dtype=np.uint8),  # Pink
            triangle_indices=sphere_data["triangle_indices"],
        ),
    )

    # Instance 3: Albedo texture with UV coordinates (right)
    print("Logging sphere with albedo texture...")
    texture = generate_checkerboard_texture()
    rr.log("world/sphere/albedo_texture", rr.Transform3D(translation=[1.5, 0.0, 0.0]))
    rr.log(
        "world/sphere/albedo_texture",
        rr.Mesh3D(
            vertex_positions=sphere_data["vertex_positions"],
            vertex_texcoords=sphere_data["vertex_texcoords"],
            albedo_texture=texture,
            triangle_indices=sphere_data["triangle_indices"],
        ),
    )

    # Instance 4: Vertex normals for smooth shading (above)
    print("Logging sphere with vertex normals...")
    rr.log("world/sphere/vertex_normals", rr.Transform3D(translation=[0.0, 1.5, 0.0]))
    rr.log(
        "world/sphere/vertex_normals",
        rr.Mesh3D(
            vertex_positions=sphere_data["vertex_positions"],
            vertex_normals=sphere_data["vertex_normals"],
            albedo_factor=np.array([100, 150, 255, 255], dtype=np.uint8),  # Light blue
            triangle_indices=sphere_data["triangle_indices"],
        ),
    )

    print("Done! All mesh variations logged to Rerun.")
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
