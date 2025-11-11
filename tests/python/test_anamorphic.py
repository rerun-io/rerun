#!/usr/bin/env python3
"""Quick test to verify anamorphic camera support."""

import numpy as np
import rerun as rr

rr.init("anamorphic_test", spawn=False)

# Create a test image with grid pattern
width, height = 640, 480
image = np.zeros((height, width, 3), dtype=np.uint8)

# Add checkerboard
for y in range(0, height, 40):
    for x in range(0, width, 40):
        if ((x // 40) + (y // 40)) % 2 == 0:
            image[y:y+40, x:x+40] = [200, 200, 200]

# Test 1: Symmetric camera (should look normal)
rr.log("camera_symmetric", rr.ViewCoordinates.RDF, static=True)
rr.log("camera_symmetric",
       rr.Pinhole(focal_length=500.0, width=width, height=height))
rr.log("camera_symmetric", rr.Image(image))

# Test 2: Anamorphic camera (should show different FOV in x vs y)
rr.log("camera_anamorphic", rr.ViewCoordinates.RDF, static=True)
rr.log("camera_anamorphic",
       rr.Pinhole(focal_length=[700.0, 400.0], width=width, height=height))
rr.log("camera_anamorphic", rr.Image(image))

# Add 3D reference points to verify projection
points = []
for x in np.linspace(-1, 1, 5):
    for y in np.linspace(-1, 1, 5):
        points.append([x, y, 3.0])

rr.log("world/points", rr.Points3D(positions=points, radii=0.05))

print("✓ Test data logged")
print("  - Check that 'camera_symmetric' shows square grid")
print("  - Check that 'camera_anamorphic' shows stretched grid (wider horizontally)")
print("  - Check that 3D points project correctly in both views")
