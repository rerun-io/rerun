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

import numpy as np
import numpy.typing as npt
import rerun as rr  # pip install rerun-sdk
import rerun.blueprint as rrb

DESCRIPTION = """
# Raw meshes
This example shows how to construct and log 3D meshes programmatically from scratch.

It generates several geometric primitives, each demonstrating different `Mesh3D` features:
- **Cube**: per-vertex colors
- **Pyramid**: UV texture coordinates with a procedural checkerboard texture
- **Sphere**: vertex normals for smooth shading
- **Icosahedron**: flat shading (no normals)

The full source code for this example is available [on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/raw_mesh).
"""


def generate_checkerboard_texture(size: int = 64, checker_size: int = 8) -> npt.NDArray[np.uint8]:
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
                texture[i, j] = [255, 200, 50]  # Gold
            else:
                texture[i, j] = [50, 50, 50]  # Dark gray
    return texture


def generate_cube() -> dict[str, npt.NDArray[np.floating] | npt.NDArray[np.unsignedinteger]]:
    """
    Generate a cube with per-vertex colors.

    Each face has vertices with different colors at the corners,
    creating a gradient effect across the cube.

    Returns a dictionary with vertex_positions, vertex_colors, and triangle_indices.
    """
    # For proper per-face coloring, we need separate vertices for each face (24 vertices total)
    # Each face has 4 vertices, and we have 6 faces
    vertices = np.array(
        [
            # Front face (z = 0.5)
            [-0.5, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
            # Back face (z = -0.5)
            [0.5, -0.5, -0.5],
            [-0.5, -0.5, -0.5],
            [-0.5, 0.5, -0.5],
            [0.5, 0.5, -0.5],
            # Right face (x = 0.5)
            [0.5, -0.5, 0.5],
            [0.5, -0.5, -0.5],
            [0.5, 0.5, -0.5],
            [0.5, 0.5, 0.5],
            # Left face (x = -0.5)
            [-0.5, -0.5, -0.5],
            [-0.5, -0.5, 0.5],
            [-0.5, 0.5, 0.5],
            [-0.5, 0.5, -0.5],
            # Top face (y = 0.5)
            [-0.5, 0.5, 0.5],
            [0.5, 0.5, 0.5],
            [0.5, 0.5, -0.5],
            [-0.5, 0.5, -0.5],
            # Bottom face (y = -0.5)
            [-0.5, -0.5, -0.5],
            [0.5, -0.5, -0.5],
            [0.5, -0.5, 0.5],
            [-0.5, -0.5, 0.5],
        ],
        dtype=np.float32,
    )

    # Colors for each face (4 vertices per face with same color)
    face_colors = [
        [255, 100, 100],  # Front - red
        [100, 255, 100],  # Back - green
        [100, 100, 255],  # Right - blue
        [255, 255, 100],  # Left - yellow
        [255, 100, 255],  # Top - magenta
        [100, 255, 255],  # Bottom - cyan
    ]
    colors = np.array([c for c in face_colors for _ in range(4)], dtype=np.uint8)

    # Two triangles per face (counter-clockwise winding)
    indices = []
    for face in range(6):
        base = face * 4
        indices.append([base, base + 1, base + 2])
        indices.append([base, base + 2, base + 3])
    indices = np.array(indices, dtype=np.uint32)

    return {
        "vertex_positions": vertices,
        "vertex_colors": colors,
        "triangle_indices": indices,
    }


def generate_pyramid() -> dict[str, npt.NDArray[np.floating] | npt.NDArray[np.unsignedinteger]]:
    """
    Generate a pyramid with UV texture coordinates.

    A four-sided pyramid with a square base, suitable for demonstrating texture mapping.

    Returns a dictionary with vertex_positions, vertex_texcoords, and triangle_indices.
    """
    # Pyramid with apex at top, base on XZ plane
    # We need separate vertices for each face for proper UV mapping
    apex = [0.0, 0.7, 0.0]
    base_corners = [
        [-0.5, -0.3, -0.5],  # back-left
        [0.5, -0.3, -0.5],  # back-right
        [0.5, -0.3, 0.5],  # front-right
        [-0.5, -0.3, 0.5],  # front-left
    ]

    vertices = []
    texcoords = []
    indices = []

    # Four triangular faces
    for i in range(4):
        next_i = (i + 1) % 4
        base_idx = len(vertices)

        # Add vertices for this face
        vertices.append(apex)
        vertices.append(base_corners[i])
        vertices.append(base_corners[next_i])

        # UV coordinates: apex at top center, base corners at bottom
        texcoords.append([0.5, 0.0])  # apex
        texcoords.append([0.0, 1.0])  # base corner 1
        texcoords.append([1.0, 1.0])  # base corner 2

        # Triangle (counter-clockwise when viewed from outside)
        indices.append([base_idx, base_idx + 2, base_idx + 1])

    # Base (two triangles)
    base_start = len(vertices)
    for corner in base_corners:
        vertices.append(corner)
    # UV for base
    texcoords.extend([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
    # Base triangles (counter-clockwise when viewed from below)
    indices.append([base_start, base_start + 1, base_start + 2])
    indices.append([base_start, base_start + 2, base_start + 3])

    return {
        "vertex_positions": np.array(vertices, dtype=np.float32),
        "vertex_texcoords": np.array(texcoords, dtype=np.float32),
        "triangle_indices": np.array(indices, dtype=np.uint32),
    }


def generate_sphere(
    subdivisions: int = 32,
) -> dict[str, npt.NDArray[np.floating] | npt.NDArray[np.unsignedinteger]]:
    """
    Generate a UV sphere with vertex normals for smooth shading.

    Creates a sphere using latitude/longitude parameterization.

    Args:
        subdivisions: Number of latitude subdivisions (longitude will be 2x this)

    Returns a dictionary with vertex_positions, vertex_normals, and triangle_indices.
    """
    if subdivisions < 3:
        msg = "Sphere subdivisions must be at least 3 to form a valid mesh."
        raise ValueError(msg)

    lat_divs = subdivisions
    lon_divs = subdivisions * 2

    vertices = []
    normals = []

    # Generate vertices and normals
    for lat in range(lat_divs + 1):
        theta = np.pi * lat / lat_divs  # 0 to pi
        sin_theta = np.sin(theta)
        cos_theta = np.cos(theta)

        for lon in range(lon_divs):
            phi = 2 * np.pi * lon / lon_divs  # 0 to 2pi
            sin_phi = np.sin(phi)
            cos_phi = np.cos(phi)

            # Position on unit sphere
            x = sin_theta * cos_phi
            y = cos_theta
            z = sin_theta * sin_phi

            vertices.append([x * 0.5, y * 0.5, z * 0.5])  # Scale to radius 0.5
            normals.append([x, y, z])  # Normal is same as position direction for unit sphere

    vertices = np.array(vertices, dtype=np.float32)
    normals = np.array(normals, dtype=np.float32)

    # Generate triangle indices
    indices = []
    for lat in range(lat_divs):
        for lon in range(lon_divs):
            next_lon = (lon + 1) % lon_divs

            first = lat * lon_divs + lon
            first_next = lat * lon_divs + next_lon
            second = (lat + 1) * lon_divs + lon
            second_next = (lat + 1) * lon_divs + next_lon

            # Two triangles per quad (counter-clockwise winding)
            indices.append([first, second, first_next])
            indices.append([first_next, second, second_next])

    indices = np.array(indices, dtype=np.uint32)

    return {
        "vertex_positions": vertices,
        "vertex_normals": normals,
        "triangle_indices": indices,
    }


def generate_icosahedron() -> dict[str, npt.NDArray[np.floating] | npt.NDArray[np.unsignedinteger]]:
    """
    Generate an icosahedron (20-sided regular polyhedron) for flat shading.

    The icosahedron is rendered without vertex normals, resulting in flat-shaded faces.

    Returns a dictionary with vertex_positions and triangle_indices.
    """
    # Golden ratio
    phi = (1 + np.sqrt(5)) / 2
    scale = 0.3  # Scale to reasonable size

    # 12 vertices of an icosahedron
    vertices = np.array(
        [
            [-1, phi, 0],
            [1, phi, 0],
            [-1, -phi, 0],
            [1, -phi, 0],
            [0, -1, phi],
            [0, 1, phi],
            [0, -1, -phi],
            [0, 1, -phi],
            [phi, 0, -1],
            [phi, 0, 1],
            [-phi, 0, -1],
            [-phi, 0, 1],
        ],
        dtype=np.float32,
    )

    # Normalize vertices to unit sphere and scale
    vertices = vertices / np.linalg.norm(vertices[0]) * scale

    # 20 triangular faces (counter-clockwise winding)
    indices = np.array(
        [
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ],
        dtype=np.uint32,
    )

    return {
        "vertex_positions": vertices,
        "triangle_indices": indices,
    }


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Logs procedurally generated 3D meshes demonstrating various Mesh3D features.",
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

    # --- Cube with per-vertex colors ---
    print("Logging cube with vertex colors...")
    cube = generate_cube()
    rr.log("world/cube", rr.Transform3D(translation=[-1.5, 0.0, 0.0]))
    rr.log(
        "world/cube",
        rr.Mesh3D(
            vertex_positions=cube["vertex_positions"],
            vertex_colors=cube["vertex_colors"],
            triangle_indices=cube["triangle_indices"],
        ),
    )

    # --- Pyramid with texture ---
    print("Logging pyramid with texture...")
    pyramid = generate_pyramid()
    texture = generate_checkerboard_texture()
    rr.log("world/pyramid", rr.Transform3D(translation=[1.5, 0.0, 0.0]))
    rr.log(
        "world/pyramid",
        rr.Mesh3D(
            vertex_positions=pyramid["vertex_positions"],
            vertex_texcoords=pyramid["vertex_texcoords"],
            albedo_texture=texture,
            triangle_indices=pyramid["triangle_indices"],
        ),
    )

    # --- Sphere with vertex normals (smooth shading) ---
    print(f"Logging sphere with {args.sphere_subdivisions} subdivisions...")
    sphere = generate_sphere(args.sphere_subdivisions)
    rr.log("world/sphere", rr.Transform3D(translation=[0.0, 0.0, 1.5]))
    rr.log(
        "world/sphere",
        rr.Mesh3D(
            vertex_positions=sphere["vertex_positions"],
            vertex_normals=sphere["vertex_normals"],
            albedo_factor=np.array([100, 150, 255, 255], dtype=np.uint8),  # Light blue
            triangle_indices=sphere["triangle_indices"],
        ),
    )

    # --- Icosahedron (flat shading, no normals) ---
    print("Logging icosahedron (flat shaded)...")
    icosahedron = generate_icosahedron()
    rr.log("world/icosahedron", rr.Transform3D(translation=[0.0, 0.0, -1.5]))
    rr.log(
        "world/icosahedron",
        rr.Mesh3D(
            vertex_positions=icosahedron["vertex_positions"],
            albedo_factor=np.array([255, 180, 100, 255], dtype=np.uint8),  # Orange
            triangle_indices=icosahedron["triangle_indices"],
        ),
    )

    print("Done! All meshes logged to Rerun.")
    rr.script_teardown(args)


if __name__ == "__main__":
    main()
