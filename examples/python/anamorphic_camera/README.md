<!--[metadata]
title = "Anamorphic Camera"
tags = ["2D", "3D", "Pinhole camera", "Anamorphic"]
description = "Demonstrates anamorphic pinhole camera support with different focal lengths (fx ≠ fy)."
thumbnail = "https://static.rerun.io/anamorphic_camera/placeholder.png"
thumbnail_dimensions = [480, 480]
channel = "main"
build_args = []
-->

# Anamorphic Pinhole Camera

This example demonstrates Rerun's support for anamorphic pinhole cameras, where the focal lengths in the x and y directions differ (fx ≠ fy).

Anamorphic cameras are used in various applications:
- Non-square pixel sensors
- Anamorphic lenses in cinematography
- Some industrial and scientific imaging systems
- Cameras with intentional optical asymmetry

## What is demonstrated

The example shows:
1. **Symmetric Camera** - Standard pinhole camera with fx = fy
2. **Anamorphic Camera** - Camera with different focal lengths (fx ≠ fy)
3. **Extreme Anamorphic** - Camera with very different focal lengths to show correct handling

Each camera views the same test pattern (checkerboard grid) and 3D reference points. The visualization demonstrates that:
- The projection correctly handles different focal lengths
- The aspect ratio and field of view are properly computed
- 3D points project correctly through anamorphic cameras

## Running

```bash
python examples/python/anamorphic_camera/main.py
```

You can also specify which cameras to show:
```bash
# Show only symmetric camera
python examples/python/anamorphic_camera/main.py --camera-type symmetric

# Show only anamorphic cameras
python examples/python/anamorphic_camera/main.py --camera-type anamorphic

# Show all (default)
python examples/python/anamorphic_camera/main.py --camera-type all
```
