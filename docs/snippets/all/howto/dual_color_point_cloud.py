"""
Demonstrates how to visualize the same point cloud with two different color schemes.

Two custom archetypes (using Rerun's Color component type) are logged on the same entity,
then a blueprint maps each color set to a separate 3D view.
"""

from __future__ import annotations

import numpy as np
import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.datatypes import ComponentSourceKind, VisualizerComponentMapping

rr.init("rerun_example_custom_color_archetypes", spawn=True)

# --- Generate a torus point cloud ---
N = 8_000
rng = np.random.default_rng(42)
theta = rng.uniform(0, 2 * np.pi, N)  # angle around the ring
phi = rng.uniform(0, 2 * np.pi, N)  # angle around the tube
tube = 3.0 + np.cos(phi)  # major radius 3, minor radius 1
positions = np.column_stack([tube * np.cos(theta), tube * np.sin(theta), np.sin(phi)])

# --- Color scheme 1: height (z-coordinate), cool-to-warm ---
z_norm = (np.sin(phi) + 1.0) / 2.0
height_rgba = np.column_stack([
    np.interp(z_norm, [0, 0.5, 1], [59, 220, 180]),  # R
    np.interp(z_norm, [0, 0.5, 1], [76, 220, 4]),  # G
    np.interp(z_norm, [0, 0.5, 1], [192, 220, 38]),  # B
    np.full(N, 255),
]).astype(np.uint8)

# --- Color scheme 2: toroidal angle, cyclic (teal → purple → orange → teal) ---
theta_norm = theta / (2 * np.pi)
spin_rgba = np.column_stack([
    np.interp(theta_norm, [0, 0.25, 0.5, 0.75, 1], [0, 120, 255, 200, 0]),  # R
    np.interp(theta_norm, [0, 0.25, 0.5, 0.75, 1], [200, 40, 140, 220, 200]),  # G
    np.interp(theta_norm, [0, 0.25, 0.5, 0.75, 1], [200, 200, 50, 60, 200]),  # B
    np.full(N, 255),
]).astype(np.uint8)

# region: log_custom_archetypes
# --- Log positions once, then each color set as a separate custom archetype ---
rr.log(
    "pointcloud",
    rr.Points3D(positions, radii=0.06),
    rr.DynamicArchetype("HeightColors", components={"colors": rr.components.ColorBatch(height_rgba)}),
    rr.DynamicArchetype("SpinColors", components={"colors": rr.components.ColorBatch(spin_rgba)}),
)
# endregion: log_custom_archetypes

# region: blueprint
# --- Blueprint: two side-by-side 3D views with different color mappings ---
blueprint = rrb.Blueprint(
    rrb.Horizontal(
        rrb.Spatial3DView(
            name="Height Colors",
            origin="/",
            overrides={
                "pointcloud": [
                    rr.Points3D.from_fields().visualizer(
                        mappings=[
                            VisualizerComponentMapping(
                                target="Points3D:colors",
                                source_kind=ComponentSourceKind.SourceComponent,
                                source_component="HeightColors:colors",
                            ),
                        ]
                    ),
                ],
            },
        ),
        rrb.Spatial3DView(
            name="Spin Colors",
            origin="/",
            overrides={
                "pointcloud": [
                    rr.Points3D.from_fields().visualizer(
                        mappings=[
                            VisualizerComponentMapping(
                                target="Points3D:colors",
                                source_kind=ComponentSourceKind.SourceComponent,
                                source_component="SpinColors:colors",
                            ),
                        ]
                    ),
                ],
            },
        ),
    ),
    collapse_panels=True,
)

rr.send_blueprint(blueprint)
# endregion: blueprint
